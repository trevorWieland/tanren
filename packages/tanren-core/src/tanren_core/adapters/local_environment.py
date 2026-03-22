"""LocalExecutionEnvironment — wraps existing fine-grained adapters."""

import asyncio
import logging
import time
import uuid
from pathlib import Path
from typing import TYPE_CHECKING, cast

from tanren_core.adapters.types import (
    AccessInfo,
    EnvironmentHandle,
    LocalEnvironmentRuntime,
    PhaseResult,
    ProvisionError,
)
from tanren_core.env.reporter import format_report
from tanren_core.errors import TRANSIENT_BACKOFF, ErrorClass, classify_error
from tanren_core.metrics import compute_plan_hash, count_unchecked_tasks
from tanren_core.schemas import Dispatch, Outcome, Phase, Result, parse_issue_from_workflow_id
from tanren_core.signals import extract_signal, map_outcome

if TYPE_CHECKING:
    from tanren_core.adapters.protocols import (
        EnvValidator,
        PostflightRunner,
        PreflightRunner,
        ProcessSpawner,
    )
    from tanren_core.adapters.remote_types import DryRunInfo, VMHandle, VMRequirements
    from tanren_core.worker_config import WorkerConfig

logger = logging.getLogger(__name__)

_PUSH_PHASES = frozenset({Phase.DO_TASK, Phase.AUDIT_TASK, Phase.RUN_DEMO, Phase.AUDIT_SPEC})


class LocalExecutionEnvironment:
    """ExecutionEnvironment backed by local subprocess adapters.

    Wraps EnvValidator, PreflightRunner, PostflightRunner, and ProcessSpawner
    into the provision/execute/teardown lifecycle.
    """

    def __init__(
        self,
        *,
        env_validator: EnvValidator,
        preflight: PreflightRunner,
        postflight: PostflightRunner,
        spawner: ProcessSpawner,
        config: WorkerConfig,
    ) -> None:
        """Initialize with local subprocess adapters for each lifecycle phase."""
        self._env_validator = env_validator
        self._preflight = preflight
        self._postflight = postflight
        self._spawner = spawner
        self._config = config

    async def dry_run(
        self,
        requirements: VMRequirements,  # noqa: ARG002 — requirements reserved for future local environment sizing
    ) -> DryRunInfo:
        """Dry-run provision — return what would happen without creating resources.

        Returns:
            DryRunInfo indicating no provisioning (local environment).
        """
        from tanren_core.adapters.remote_types import DryRunInfo, VMProvider  # noqa: PLC0415

        return DryRunInfo(
            provider=VMProvider.MANUAL,
            would_provision=False,
        )

    async def provision(self, dispatch: Dispatch, config: WorkerConfig) -> EnvironmentHandle:
        """Env validation -> preflight -> return handle.

        Returns:
            EnvironmentHandle for local execution.

        Raises:
            ProvisionError: If environment validation or preflight checks fail.
        """
        issue = _parse_issue(dispatch.workflow_id, project=dispatch.project)
        worktree_path = Path(config.github_dir) / f"{dispatch.project}-wt-{issue}"
        spec_folder_path = worktree_path / dispatch.spec_folder

        # 1. Environment validation
        env_report, task_env = await self._env_validator.load_and_validate(worktree_path)
        if not env_report.passed:
            result = Result(
                workflow_id=dispatch.workflow_id,
                phase=dispatch.phase,
                outcome=Outcome.ERROR,
                signal=None,
                exit_code=-1,
                duration_secs=0,
                gate_output=None,
                tail_output=format_report(
                    env_report, dispatch.project, str(worktree_path / "tanren.yml")
                ),
                unchecked_tasks=0,
                plan_hash="00000000",
                spec_modified=False,
            )
            raise ProvisionError(result)

        # 2. Preflight checks
        preflight_result = await self._preflight.run(
            worktree_path, dispatch.branch, spec_folder_path, dispatch.phase.value
        )

        if not preflight_result.passed:
            result = Result(
                workflow_id=dispatch.workflow_id,
                phase=dispatch.phase,
                outcome=Outcome.ERROR,
                signal=None,
                exit_code=-1,
                duration_secs=0,
                gate_output=None,
                tail_output=preflight_result.error,
                unchecked_tasks=0,
                plan_hash="00000000",
                spec_modified=False,
            )
            raise ProvisionError(result, preflight_result)

        if preflight_result.repairs:
            logger.info("Preflight repairs: %s", preflight_result.repairs)

        # 3. Return handle
        return EnvironmentHandle(
            env_id=str(uuid.uuid4()),
            worktree_path=worktree_path,
            branch=dispatch.branch,
            project=dispatch.project,
            runtime=LocalEnvironmentRuntime(
                preflight_result=preflight_result,
                task_env=task_env,
                env_report=env_report,
            ),
        )

    async def execute(
        self,
        handle: EnvironmentHandle,
        dispatch: Dispatch,
        config: WorkerConfig,
        *,
        dispatch_stem: str = "",  # noqa: ARG002 — required by protocol interface
    ) -> PhaseResult:
        """Heartbeat start -> retry loop -> plan metrics -> postflight -> heartbeat stop.

        Returns:
            PhaseResult with outcome, signal, and metrics.

        Raises:
            RuntimeError: If the handle does not contain a local runtime.
        """
        spec_folder_path = handle.worktree_path / dispatch.spec_folder
        if handle.runtime.kind != "local":
            raise RuntimeError("LocalExecutionEnvironment requires local runtime handle")
        local_runtime = cast("LocalEnvironmentRuntime", handle.runtime)

        start = time.monotonic()
        transient_retries = 0
        try:
            while True:
                # Spawn process
                proc_result = await self._spawner.spawn(
                    dispatch,
                    handle.worktree_path,
                    config,
                    task_env=local_runtime.task_env or None,
                )

                # Log process result for agent phases
                if dispatch.phase not in (Phase.GATE, Phase.SETUP, Phase.CLEANUP):
                    stdout_preview = (proc_result.stdout or "")[:500]
                    logger.info(
                        "Process result: exit=%d duration=%ds "
                        "timed_out=%s stdout_len=%d stdout=%.200s",
                        proc_result.exit_code,
                        proc_result.duration_secs,
                        proc_result.timed_out,
                        len(proc_result.stdout or ""),
                        stdout_preview,
                    )

                # Extract signal
                command_name = dispatch.phase.value
                raw_signal = extract_signal(
                    dispatch.phase, command_name, spec_folder_path, proc_result.stdout
                )

                # Map outcome
                outcome, signal_val = map_outcome(
                    dispatch.phase,
                    raw_signal,
                    proc_result.exit_code,
                    proc_result.timed_out,
                )

                # Transient error retry
                if outcome in (Outcome.ERROR, Outcome.TIMEOUT):
                    stderr_text = ""
                    error_class = classify_error(
                        proc_result.exit_code,
                        proc_result.stdout or "",
                        stderr_text,
                        signal_val,
                    )
                    if error_class == ErrorClass.TRANSIENT and transient_retries < 3:
                        transient_retries += 1
                        backoff = TRANSIENT_BACKOFF[transient_retries - 1]
                        logger.warning(
                            "Transient error (attempt %d/3), retrying in %ds",
                            transient_retries,
                            backoff,
                        )
                        await asyncio.sleep(backoff)
                        continue
                    elif error_class == ErrorClass.AMBIGUOUS and transient_retries < 1:
                        transient_retries += 1
                        logger.warning("Ambiguous error, retrying once in 10s")
                        await asyncio.sleep(10)
                        continue

                # Not retrying — break out of loop
                break

            duration = int(time.monotonic() - start)

            # Compute plan.md metrics
            plan_path = spec_folder_path / "plan.md"
            unchecked = await count_unchecked_tasks(plan_path)
            plan_hash = await compute_plan_hash(plan_path)

            # Post-flight integrity checks
            postflight_result = None
            if dispatch.phase in _PUSH_PHASES:
                preflight = local_runtime.preflight_result
                postflight_result = await self._postflight.run(
                    handle.worktree_path,
                    dispatch.branch,
                    dispatch.phase.value,
                    preflight.file_hashes if preflight else {},
                    preflight.file_backups if preflight else {},
                    skip_push=(outcome in (Outcome.ERROR, Outcome.TIMEOUT)),
                )

            return PhaseResult(
                outcome=outcome,
                signal=signal_val,
                exit_code=proc_result.exit_code,
                stdout=proc_result.stdout,
                duration_secs=duration,
                preflight_passed=True,
                postflight_result=postflight_result,
                env_report=local_runtime.env_report,
                gate_output=None,  # Manager builds this
                unchecked_tasks=unchecked,
                plan_hash=plan_hash,
                retries=transient_retries,
            )

        finally:
            pass

    async def get_access_info(self, handle: EnvironmentHandle) -> AccessInfo:
        """Return local worktree path. No SSH/VSCode for local."""
        return AccessInfo(working_dir=str(handle.worktree_path), status="local")

    async def release_vm(self, vm_handle: VMHandle) -> None:
        """No-op for local — no VM to release."""

    async def teardown(self, handle: EnvironmentHandle) -> None:
        """No-op for local (heartbeat already stopped in execute).

        Future Docker/VM envs would destroy containers here.
        """
        pass


def _parse_issue(workflow_id: str, *, project: str | None = None) -> str:
    """Extract issue identifier from workflow_id.

    Returns:
        Issue identifier parsed from the workflow_id.
    """
    return parse_issue_from_workflow_id(workflow_id, project=project)
