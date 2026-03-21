"""Run lifecycle service — provision, execute, teardown, full lifecycle, status."""

from __future__ import annotations

import asyncio
import contextlib
import logging
import uuid
from datetime import UTC, datetime
from pathlib import Path

from tanren_api.errors import ConflictError, NotFoundError, ServiceError
from tanren_api.models import (
    CheckpointDetail,
    CheckpointSummary,
    DispatchAccepted,
    DispatchRunStatus,
    ExecuteRequest,
    ProvisionRequest,
    ResumeAccepted,
    RunEnvironment,
    RunEnvironmentStatus,
    RunExecuteAccepted,
    RunFullRequest,
    RunStatus,
    RunTeardownAccepted,
)
from tanren_api.state import APIStateStore, DispatchRecord, EnvironmentRecord
from tanren_core.adapters.protocols import ExecutionEnvironment
from tanren_core.adapters.types import EnvironmentHandle, RemoteEnvironmentRuntime
from tanren_core.config import Config
from tanren_core.ipc import list_checkpoints as list_checkpoints_ipc
from tanren_core.ipc import read_checkpoint
from tanren_core.roles import RoleName
from tanren_core.roles_config import load_roles_config
from tanren_core.schemas import Dispatch, Outcome, Phase

logger = logging.getLogger(__name__)

_EXECUTE_FROM = frozenset({RunEnvironmentStatus.PROVISIONED, RunEnvironmentStatus.COMPLETED})
_TEARDOWN_FROM = frozenset({
    RunEnvironmentStatus.PROVISIONING,
    RunEnvironmentStatus.PROVISIONED,
    RunEnvironmentStatus.EXECUTING,
    RunEnvironmentStatus.COMPLETED,
    RunEnvironmentStatus.FAILED,
})

_PRIOR_TASK_TIMEOUT: float = 30.0

_COMPLETED_DISPATCH_OUTCOMES = frozenset({Outcome.SUCCESS, Outcome.FAIL, Outcome.BLOCKED})
_COMPLETED_ENV_OUTCOMES = frozenset({Outcome.SUCCESS, Outcome.FAIL, Outcome.BLOCKED})


def _now() -> str:
    return datetime.now(UTC).isoformat()


class RunService:
    """Service for run lifecycle management."""

    def __init__(
        self,
        store: APIStateStore,
        config: Config | None = None,
        execution_env: ExecutionEnvironment | None = None,
    ) -> None:
        """Initialize with dependencies."""
        self._store = store
        self._config = config
        self._execution_env = execution_env

    def _require_config(self) -> Config:
        """Return config or raise if unavailable.

        Returns:
            Config: The application configuration.

        Raises:
            ServiceError: If configuration is not set.
        """
        if self._config is None:
            raise ServiceError("Configuration unavailable — WM_* environment variables not set")
        return self._config

    def _require_execution_env(self) -> ExecutionEnvironment:
        """Return execution environment or raise if unavailable.

        Returns:
            ExecutionEnvironment: The configured execution environment.

        Raises:
            ServiceError: If execution environment is not configured.
        """
        if self._execution_env is None:
            raise ServiceError("Remote execution environment not configured")
        return self._execution_env

    async def provision(self, body: ProvisionRequest) -> RunEnvironment:
        """Provision a remote execution environment (non-blocking).

        Returns:
            RunEnvironment: Provisioning environment with tracking env_id.
        """
        config = self._require_config()
        execution_env = self._require_execution_env()

        env_id = str(uuid.uuid4())

        roles = load_roles_config(config.roles_config_path)
        resolved_tool = roles.resolve(RoleName.DEFAULT)

        dispatch = Dispatch(
            workflow_id=f"run-{uuid.uuid4().hex[:8]}",
            project=body.project,
            phase=Phase.DO_TASK,
            branch=body.branch,
            spec_folder="",
            cli=resolved_tool.cli,
            auth=resolved_tool.auth,
            timeout=1800,
            environment_profile=body.environment_profile,
        )

        record = EnvironmentRecord(
            env_id=env_id,
            handle=None,
            status=RunEnvironmentStatus.PROVISIONING,
            started_at=_now(),
        )
        await self._store.add_environment(record)

        async def _provision_background() -> None:
            handle: EnvironmentHandle | None = None
            try:
                handle = await execution_env.provision(dispatch, config)
                runtime = handle.runtime
                if not isinstance(runtime, RemoteEnvironmentRuntime):
                    raise ServiceError("Provisioned environment is not a remote runtime")
                updated = await self._store.try_transition_environment(
                    env_id,
                    from_statuses=frozenset({RunEnvironmentStatus.PROVISIONING}),
                    to_status=RunEnvironmentStatus.PROVISIONED,
                    handle=handle,
                    vm_id=runtime.vm_handle.vm_id,
                    host=runtime.vm_handle.host,
                )
                if updated is not None:
                    handle = None  # Persisted — suppress finally cleanup
            except asyncio.CancelledError:
                raise
            except Exception:
                handle = None  # Error handler owns cleanup
                logger.exception("Provision failed for %s", env_id)
                await self._store.try_transition_environment(
                    env_id,
                    from_statuses=frozenset({RunEnvironmentStatus.PROVISIONING}),
                    to_status=RunEnvironmentStatus.FAILED,
                    outcome=Outcome.ERROR,
                    completed_at=_now(),
                )
            finally:
                if handle is not None:
                    logger.warning("Cleaning up orphaned provision for %s", env_id)
                    inner = asyncio.ensure_future(execution_env.teardown(handle))
                    try:
                        await asyncio.shield(inner)
                    except asyncio.CancelledError, Exception:
                        with contextlib.suppress(asyncio.CancelledError, Exception):
                            await inner

        task = asyncio.create_task(_provision_background())
        await self._store.update_environment(env_id, task=task)

        return RunEnvironment(
            env_id=env_id,
            vm_id="",
            host="",
            status=RunEnvironmentStatus.PROVISIONING,
        )

    async def execute(self, env_id: str, body: ExecuteRequest) -> RunExecuteAccepted:
        """Execute a phase against a provisioned environment.

        Returns:
            RunExecuteAccepted: Accepted response with env_id and dispatch_id.

        Raises:
            NotFoundError: If the environment is not found.
            ConflictError: If the environment is still provisioning, has a project mismatch,
                or cannot transition to executing.
        """
        config = self._require_config()
        execution_env = self._require_execution_env()

        record = await self._store.get_environment(env_id)
        if record is None:
            raise NotFoundError(f"Environment {env_id} not found")
        if record.handle is None:
            raise ConflictError(f"Environment {env_id} is still provisioning")
        if body.project != record.handle.project:
            raise ConflictError(
                f"Project mismatch: environment provisioned for '{record.handle.project}', "
                f"request specifies '{body.project}'"
            )

        dispatch_id = f"exec-{uuid.uuid4().hex[:8]}"

        dispatch = Dispatch(
            workflow_id=dispatch_id,
            project=body.project,
            phase=body.phase,
            branch=record.handle.branch,
            spec_folder=body.spec_path,
            cli=body.cli,
            auth=body.auth,
            model=body.model,
            timeout=body.timeout,
            context=body.context,
            gate_cmd=body.gate_cmd,
        )

        gate = asyncio.Event()
        handle = record.handle

        async def _execute_background() -> None:
            await gate.wait()
            try:
                result = await execution_env.execute(handle, dispatch, config)
                env_status = (
                    RunEnvironmentStatus.COMPLETED
                    if result.outcome in _COMPLETED_ENV_OUTCOMES
                    else RunEnvironmentStatus.FAILED
                )
                await self._store.update_environment(
                    env_id,
                    status=env_status,
                    outcome=result.outcome,
                    completed_at=_now(),
                )
            except asyncio.CancelledError:
                raise
            except Exception:
                logger.exception("Execute failed for env %s", env_id)
                await self._store.update_environment(
                    env_id,
                    status=RunEnvironmentStatus.FAILED,
                    outcome=Outcome.ERROR,
                    completed_at=_now(),
                )

        task = asyncio.create_task(_execute_background())
        updated = await self._store.try_transition_environment(
            env_id,
            from_statuses=_EXECUTE_FROM,
            to_status=RunEnvironmentStatus.EXECUTING,
            phase=body.phase,
            dispatch_id=dispatch_id,
            started_at=_now(),
            outcome=None,
            completed_at=None,
            task=task,
        )
        if updated is None:
            task.cancel()
            with contextlib.suppress(asyncio.CancelledError):
                await task
            raise ConflictError(f"Environment {env_id} cannot transition to executing")
        gate.set()

        return RunExecuteAccepted(env_id=env_id, dispatch_id=dispatch_id)

    async def teardown(self, env_id: str) -> RunTeardownAccepted:
        """Teardown a provisioned environment.

        Returns:
            RunTeardownAccepted: Confirmation that teardown has been initiated.

        Raises:
            NotFoundError: If the environment is not found.
            ConflictError: If the environment is already being torn down.
        """
        execution_env = self._require_execution_env()

        record = await self._store.get_environment(env_id)
        if record is None:
            raise NotFoundError(f"Environment {env_id} not found")

        if record.status not in _TEARDOWN_FROM:
            raise ConflictError(f"Environment {env_id} is already being torn down")

        # Claim ownership of teardown BEFORE cancelling any running task.
        updated = await self._store.try_transition_environment(
            env_id,
            from_statuses=_TEARDOWN_FROM,
            to_status=RunEnvironmentStatus.TEARING_DOWN,
        )
        if updated is None:
            raise ConflictError(f"Environment {env_id} is already being torn down")

        await self._store.cancel_environment_task(env_id)

        pre = await self._store.get_environment(env_id)
        prior_task = pre.task if pre is not None else None

        async def _teardown_background() -> None:
            if prior_task is not None and not prior_task.done():
                logger.debug("Waiting for prior task before teardown for %s", env_id)
                with contextlib.suppress(TimeoutError, asyncio.CancelledError, Exception):
                    await asyncio.wait_for(asyncio.shield(prior_task), timeout=_PRIOR_TASK_TIMEOUT)

            current = await self._store.get_environment(env_id)
            handle = current.handle if current is not None else None
            if handle is None:
                await self._store.remove_environment(env_id)
                return
            inner = asyncio.ensure_future(execution_env.teardown(handle))
            try:
                await asyncio.shield(inner)
            except asyncio.CancelledError, Exception:
                with contextlib.suppress(asyncio.CancelledError, Exception):
                    await inner
                logger.warning("Teardown failed for env %s", env_id)
            finally:
                await self._store.remove_environment(env_id)

        task = asyncio.create_task(_teardown_background())
        await self._store.update_environment(env_id, task=task)

        return RunTeardownAccepted(env_id=env_id)

    async def full(self, body: RunFullRequest) -> DispatchAccepted:
        """Full lifecycle: provision, execute, teardown. Returns ID for polling.

        Returns:
            DispatchAccepted: Accepted response with dispatch_id for polling.
        """
        config = self._require_config()
        execution_env = self._require_execution_env()

        workflow_id = f"full-{uuid.uuid4().hex[:8]}"

        dispatch = Dispatch(
            workflow_id=workflow_id,
            project=body.project,
            phase=body.phase,
            branch=body.branch,
            spec_folder=body.spec_path,
            cli=body.cli,
            auth=body.auth,
            timeout=body.timeout,
            environment_profile=body.environment_profile,
            context=body.context,
            gate_cmd=body.gate_cmd,
        )

        async def _full_lifecycle() -> None:
            handle: EnvironmentHandle | None = None
            try:
                handle = await execution_env.provision(dispatch, config)
                if not isinstance(handle.runtime, RemoteEnvironmentRuntime):
                    raise ServiceError("Provisioned environment is not a remote runtime")
                vm_handle = handle.runtime.vm_handle

                env_record = EnvironmentRecord(
                    env_id=handle.env_id,
                    handle=handle,
                    status=RunEnvironmentStatus.EXECUTING,
                    phase=body.phase,
                    vm_id=vm_handle.vm_id,
                    host=vm_handle.host,
                    dispatch_id=workflow_id,
                    started_at=_now(),
                )
                await self._store.add_environment(env_record)

                result = await execution_env.execute(handle, dispatch, config)
                dispatch_status = (
                    DispatchRunStatus.COMPLETED
                    if result.outcome in _COMPLETED_DISPATCH_OUTCOMES
                    else DispatchRunStatus.FAILED
                )
                env_status = (
                    RunEnvironmentStatus.COMPLETED
                    if result.outcome in _COMPLETED_ENV_OUTCOMES
                    else RunEnvironmentStatus.FAILED
                )
                await self._store.try_transition_dispatch(
                    workflow_id,
                    from_statuses=frozenset({DispatchRunStatus.RUNNING}),
                    to_status=dispatch_status,
                    outcome=result.outcome,
                    completed_at=_now(),
                )
                await self._store.update_environment(
                    handle.env_id,
                    status=env_status,
                    outcome=result.outcome,
                    completed_at=_now(),
                )
            except asyncio.CancelledError:
                raise
            except Exception:
                logger.exception("Full lifecycle failed for %s", workflow_id)
                await self._store.try_transition_dispatch(
                    workflow_id,
                    from_statuses=frozenset({DispatchRunStatus.RUNNING}),
                    to_status=DispatchRunStatus.FAILED,
                    outcome=Outcome.ERROR,
                    completed_at=_now(),
                )
            finally:
                if handle is not None:
                    inner = asyncio.ensure_future(execution_env.teardown(handle))
                    try:
                        await asyncio.shield(inner)
                    except asyncio.CancelledError, Exception:
                        with contextlib.suppress(asyncio.CancelledError, Exception):
                            await inner
                        logger.warning("Teardown failed for %s", workflow_id)
                    finally:
                        try:
                            await self._store.remove_environment(handle.env_id)
                        except asyncio.CancelledError, Exception:
                            logger.warning("Failed to remove environment %s", handle.env_id)

        dispatch_record = DispatchRecord(
            dispatch_id=workflow_id,
            dispatch=dispatch,
            status=DispatchRunStatus.RUNNING,
            created_at=_now(),
            started_at=_now(),
        )
        await self._store.add_dispatch(dispatch_record)

        task = asyncio.create_task(_full_lifecycle())
        await self._store.update_dispatch(workflow_id, task=task)

        return DispatchAccepted(dispatch_id=workflow_id)

    async def status(self, env_id: str) -> RunStatus:
        """Poll status of a running environment.

        Returns:
            RunStatus: Current status of the environment.

        Raises:
            NotFoundError: If the environment is not found.
        """
        record = await self._store.get_environment(env_id)
        if record is None:
            raise NotFoundError(f"Environment {env_id} not found")

        duration_secs = None
        if record.started_at is not None:
            try:
                started = datetime.fromisoformat(record.started_at)
                duration_secs = int((datetime.now(UTC) - started).total_seconds())
            except ValueError, TypeError:
                pass

        return RunStatus(
            env_id=env_id,
            status=record.status,
            phase=record.phase,
            outcome=record.outcome,
            started_at=record.started_at,
            duration_secs=duration_secs,
            vm_id=record.vm_id or None,
            host=record.host or None,
        )

    async def resume(self, workflow_id: str) -> ResumeAccepted:
        """Resume a checkpointed dispatch in a background task.

        Returns:
            ResumeAccepted with the workflow_id.

        Raises:
            NotFoundError: If no checkpoint exists for the workflow.
        """
        config = self._require_config()
        execution_env = self._require_execution_env()
        checkpoints_dir = Path(config.checkpoints_dir)
        checkpoint = await read_checkpoint(checkpoints_dir, workflow_id)
        if checkpoint is None:
            raise NotFoundError(f"No checkpoint found for workflow: {workflow_id}")

        async def _resume_background() -> None:
            from tanren_core.manager import (  # noqa: PLC0415 — heavy import
                WorkerManager,
            )

            manager = WorkerManager(config=config, execution_env=execution_env)
            try:
                await manager.resume_dispatch(workflow_id)
            except Exception:
                logger.exception("Background resume failed for %s", workflow_id)

        _task = asyncio.create_task(_resume_background())  # noqa: RUF006 — fire-and-forget background task

        return ResumeAccepted(workflow_id=workflow_id)

    async def get_checkpoints(self) -> list[CheckpointSummary]:
        """List all active checkpoints.

        Returns:
            List of CheckpointSummary instances.
        """
        config = self._require_config()
        checkpoints_dir = Path(config.checkpoints_dir)
        checkpoints = await list_checkpoints_ipc(checkpoints_dir)
        return [
            CheckpointSummary(
                workflow_id=cp.workflow_id,
                stage=cp.stage.value,
                retry_count=cp.retry_count,
                vm_id=cp.vm_id,
                last_error=cp.last_error,
                failure_count=cp.failure_count,
                created_at=cp.created_at,
                updated_at=cp.updated_at,
            )
            for cp in checkpoints
        ]

    async def get_checkpoint(self, workflow_id: str) -> CheckpointDetail:
        """Get checkpoint details for a workflow.

        Returns:
            CheckpointDetail for the workflow.

        Raises:
            NotFoundError: If no checkpoint exists for the workflow.
        """
        config = self._require_config()
        checkpoints_dir = Path(config.checkpoints_dir)
        cp = await read_checkpoint(checkpoints_dir, workflow_id)
        if cp is None:
            raise NotFoundError(f"No checkpoint found for workflow: {workflow_id}")
        return CheckpointDetail(
            workflow_id=cp.workflow_id,
            stage=cp.stage.value,
            dispatch_json=cp.dispatch_json,
            worktree_path=cp.worktree_path,
            dispatch_stem=cp.dispatch_stem,
            vm_id=cp.vm_id,
            environment_profile=cp.environment_profile,
            workspace_remote_path=cp.workspace_remote_path,
            phase_result_json=cp.phase_result_json,
            dispatch_start_utc=cp.dispatch_start_utc,
            retry_count=cp.retry_count,
            last_error=cp.last_error,
            failure_count=cp.failure_count,
            created_at=cp.created_at,
            updated_at=cp.updated_at,
        )
