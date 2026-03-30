"""Run service — step-by-step provision/execute/teardown via job queue."""

from __future__ import annotations

import logging
import time
from typing import TYPE_CHECKING

from tanren_api.errors import NotFoundError, ServiceError
from tanren_api.models import (
    DispatchAccepted,
    ExecuteRequest,
    ProvisionRequest,
    RunEnvironment,
    RunEnvironmentStatus,
    RunExecuteAccepted,
    RunFullRequest,
    RunStatus,
    RunTeardownAccepted,
)
from tanren_core.dispatch_orchestrator import (
    DispatchGuardError,
    create_dispatch,
    enqueue_execute_step,
    enqueue_teardown_step,
    get_provision_result,
)
from tanren_core.schemas import Cli, Dispatch, Outcome, Phase
from tanren_core.store.enums import (
    DispatchMode,
    DispatchStatus,
    StepStatus,
    StepType,
)
from tanren_core.store.payloads import ProvisionResult
from tanren_core.store.protocols import EventStore, JobQueue, StateStore
from tanren_core.store.views import DispatchView

if TYPE_CHECKING:
    from tanren_core.worker_config import WorkerConfig

logger = logging.getLogger(__name__)


class RunService:
    """Stateless run service — enqueues steps, reads from state store."""

    def __init__(
        self,
        *,
        event_store: EventStore,
        job_queue: JobQueue,
        state_store: StateStore,
        config: WorkerConfig | None = None,
    ) -> None:
        """Initialize with store dependencies only."""
        self._event_store = event_store
        self._job_queue = job_queue
        self._state_store = state_store
        self._config = config

    async def provision(self, body: ProvisionRequest, user_id: str = "") -> RunEnvironment:
        """Enqueue a provision step (manual mode — user drives each step).

        Returns:
            RunEnvironment with env_id.
        """
        req = body
        epoch = time.time_ns()
        workflow_id = f"wf-{req.project}-run-{epoch}"

        dispatch = Dispatch(
            workflow_id=workflow_id,
            project=req.project,
            phase=Phase.DO_TASK,
            branch=req.branch,
            spec_folder=".",
            cli=Cli.CLAUDE,
            timeout=1800,
            environment_profile=req.resolved_profile.name,
            resolved_profile=req.resolved_profile,
            project_env=req.project_env,
            cloud_secrets=req.cloud_secrets,
            required_secrets=req.required_secrets,
        )

        result = await create_dispatch(
            dispatch=dispatch,
            mode=DispatchMode.MANUAL,
            event_store=self._event_store,
            job_queue=self._job_queue,
            state_store=self._state_store,
            user_id=user_id,
            preserve_on_failure=True,
        )

        return RunEnvironment(
            env_id=result.dispatch_id,
            dispatch_id=result.dispatch_id,
            status=RunEnvironmentStatus.PROVISIONING,
        )

    async def execute(
        self, env_id: str, body: ExecuteRequest, *, user_id: str = "", is_admin: bool = False
    ) -> RunExecuteAccepted:
        """Enqueue an execute step for a provisioned environment.

        Args:
            env_id: Environment ID.
            body: Execute request parameters.
            user_id: Caller's user ID for ownership check.
            is_admin: If True, bypass ownership check.

        Returns:
            RunExecuteAccepted with dispatch_id.

        Raises:
            NotFoundError: If not found or not owned by caller.
        """
        dispatch_view = await self._find_dispatch_for_env(env_id)
        if dispatch_view is None:
            raise NotFoundError(f"Environment {env_id} not found")
        if not is_admin and user_id and dispatch_view.user_id != user_id:
            raise NotFoundError(f"Environment {env_id} not found")
        if dispatch_view.status == DispatchStatus.CANCELLED:
            raise ServiceError(f"Dispatch {dispatch_view.dispatch_id} is cancelled")

        # Get the provision step's result to extract the handle
        prov_result = await self._get_provision_result(dispatch_view.dispatch_id)

        dispatch = dispatch_view.dispatch

        # Resolve cli/auth from roles.yml when not explicitly provided
        cli, auth, model = self._resolve_execute_cli(body)

        # Resolve gate_cmd from profile defaults when not explicitly provided
        gate_cmd = body.gate_cmd
        if body.phase == Phase.GATE and not gate_cmd and self._config is not None:
            from tanren_core.dispatch_resolver import resolve_gate_cmd

            gate_cmd = resolve_gate_cmd(
                self._config, body.project, dispatch.environment_profile, body.phase, gate_cmd
            )

        # Build a new Dispatch from the execute request body, preserving
        # project/branch/profile metadata from the provision-time dispatch.
        exec_dispatch = Dispatch(
            workflow_id=dispatch_view.dispatch_id,
            project=dispatch.project,
            branch=dispatch.branch,
            environment_profile=dispatch.environment_profile,
            resolved_profile=dispatch.resolved_profile,
            phase=body.phase,
            spec_folder=body.spec_path,
            cli=cli,
            auth=auth,
            model=model,
            timeout=body.timeout,
            context=body.context,
            gate_cmd=gate_cmd,
        )

        try:
            await enqueue_execute_step(
                dispatch_id=dispatch_view.dispatch_id,
                exec_dispatch=exec_dispatch,
                handle=prov_result.handle,
                job_queue=self._job_queue,
                state_store=self._state_store,
            )
        except DispatchGuardError as exc:
            raise ServiceError(str(exc)) from exc

        return RunExecuteAccepted(
            env_id=dispatch_view.dispatch_id, dispatch_id=dispatch_view.dispatch_id
        )

    async def teardown(
        self, env_id: str, *, user_id: str = "", is_admin: bool = False
    ) -> RunTeardownAccepted:
        """Enqueue a teardown step.

        Args:
            env_id: Environment ID.
            user_id: Caller's user ID for ownership check.
            is_admin: If True, bypass ownership check.

        Returns:
            RunTeardownAccepted with dispatch_id.

        Raises:
            NotFoundError: If not found or not owned by caller.
        """
        dispatch_view = await self._find_dispatch_for_env(env_id)
        if dispatch_view is None:
            raise NotFoundError(f"Environment {env_id} not found")
        if not is_admin and user_id and dispatch_view.user_id != user_id:
            raise NotFoundError(f"Environment {env_id} not found")

        prov_result = await self._get_provision_result(dispatch_view.dispatch_id)

        try:
            await enqueue_teardown_step(
                dispatch_id=dispatch_view.dispatch_id,
                dispatch=dispatch_view.dispatch,
                handle=prov_result.handle,
                job_queue=self._job_queue,
                state_store=self._state_store,
            )
        except DispatchGuardError as exc:
            raise ServiceError(str(exc)) from exc

        return RunTeardownAccepted(
            env_id=dispatch_view.dispatch_id, dispatch_id=dispatch_view.dispatch_id
        )

    async def full(self, body: RunFullRequest, user_id: str = "") -> DispatchAccepted:
        """Enqueue a full dispatch lifecycle (auto-chained provision -> execute -> teardown).

        Returns:
            DispatchAccepted with workflow_id.
        """
        from tanren_api.models import DispatchRequest
        from tanren_api.services.dispatch_lifecycle import create_dispatch_from_request

        dispatch_req = DispatchRequest(
            phase=body.phase,
            project=body.project,
            branch=body.branch,
            spec_folder=body.spec_path,
            cli=body.cli,
            auth=body.auth,
            model=body.model,
            timeout=body.timeout,
            context=body.context,
            gate_cmd=body.gate_cmd,
            environment_profile=body.environment_profile,
            resolved_profile=body.resolved_profile,
            project_env=body.project_env,
            cloud_secrets=body.cloud_secrets,
            required_secrets=body.required_secrets,
        )

        return await create_dispatch_from_request(
            body=dispatch_req,
            event_store=self._event_store,
            job_queue=self._job_queue,
            state_store=self._state_store,
            user_id=user_id,
        )

    async def status(self, env_id: str, *, user_id: str = "", is_admin: bool = False) -> RunStatus:
        """Check status of a run environment.

        Args:
            env_id: Environment ID.
            user_id: Caller's user ID for ownership check.
            is_admin: If True, bypass ownership check.

        Returns:
            RunStatus with current state.

        Raises:
            NotFoundError: If not found or not owned by caller.
        """
        dispatch_view = await self._find_dispatch_for_env(env_id)
        if dispatch_view is None:
            raise NotFoundError(f"Environment {env_id} not found")
        if not is_admin and user_id and dispatch_view.user_id != user_id:
            raise NotFoundError(f"Environment {env_id} not found")

        steps = await self._state_store.get_steps_for_dispatch(dispatch_view.dispatch_id)

        # Derive environment status from step states.
        # Use the latest execute step for failure detection (not cumulative).
        env_status = RunEnvironmentStatus.PROVISIONING
        for step in steps:
            if step.step_type == StepType.PROVISION and step.status == StepStatus.COMPLETED:
                env_status = RunEnvironmentStatus.PROVISIONED
            elif step.step_type == StepType.EXECUTE and step.status == StepStatus.RUNNING:
                env_status = RunEnvironmentStatus.EXECUTING
            elif step.step_type == StepType.EXECUTE and step.status == StepStatus.COMPLETED:
                env_status = RunEnvironmentStatus.COMPLETED
            elif step.step_type == StepType.TEARDOWN:
                env_status = RunEnvironmentStatus.TEARING_DOWN
                if step.status == StepStatus.COMPLETED:
                    env_status = RunEnvironmentStatus.COMPLETED
            if step.status == StepStatus.FAILED:
                env_status = RunEnvironmentStatus.FAILED

        # Check dispatch-level status for cancellation and outcome
        is_cancelled = dispatch_view.status == DispatchStatus.CANCELLED
        has_error_outcome = dispatch_view.outcome in (Outcome.ERROR, Outcome.TIMEOUT)
        if is_cancelled or (has_error_outcome and env_status != RunEnvironmentStatus.FAILED):
            env_status = RunEnvironmentStatus.FAILED

        # Derive outcome from execute steps when dispatch-level outcome is
        # not set (MANUAL mode dispatches never auto-complete).
        outcome = dispatch_view.outcome
        if outcome is None:
            exec_steps = [s for s in steps if s.step_type == StepType.EXECUTE and s.result_json]
            if exec_steps and exec_steps[-1].result_json is not None:
                from tanren_core.store.payloads import ExecuteResult

                last = ExecuteResult.model_validate_json(exec_steps[-1].result_json)
                outcome = last.outcome

        # Reclassify env_status based on derived outcome — a "completed" step
        # with an error/timeout outcome should show as FAILED, not COMPLETED.
        if outcome in (Outcome.ERROR, Outcome.TIMEOUT) and env_status not in (
            RunEnvironmentStatus.FAILED,
            RunEnvironmentStatus.TEARING_DOWN,
        ):
            env_status = RunEnvironmentStatus.FAILED

        return RunStatus(
            env_id=env_id,
            dispatch_id=dispatch_view.dispatch_id,
            status=env_status,
            outcome=outcome,
        )

    # ── Internal helpers ──────────────────────────────────────────────────

    def _resolve_execute_cli(self, body: ExecuteRequest) -> tuple:
        """Resolve CLI, auth, and model for an execute step."""
        from tanren_core.dispatch_builder import resolve_cli_auth

        if body.cli is not None:
            # Explicit CLI — resolve auth/model defaults
            from tanren_core.roles import AuthMode

            auth = body.auth or AuthMode.API_KEY
            return body.cli, auth, body.model

        if self._config is None:
            if body.phase == Phase.GATE:
                # GATE always uses BASH — no roles.yml needed
                from tanren_core.roles import AuthMode as _AM

                return Cli.BASH, body.auth or _AM.API_KEY, body.model
            raise ServiceError("WorkerConfig required for CLI auto-resolution")

        # Use the shared builder resolution (handles roles.yml lookup, etc.)
        cli, auth, model = resolve_cli_auth(
            config=self._config,
            phase=body.phase,
            cli=body.cli,
            auth=body.auth,
            model=body.model,
        )
        return cli, auth, model

    async def _get_provision_result(self, dispatch_id: str) -> ProvisionResult:
        """Get provision result, raising ServiceError on failure."""
        try:
            return await get_provision_result(self._state_store, dispatch_id)
        except ValueError as exc:
            raise ServiceError(str(exc)) from exc

    async def _find_dispatch_for_env(self, env_id: str) -> DispatchView | None:
        """Find a dispatch by env_id.

        For the queue model, env_id is used as dispatch_id lookup.
        """
        # Try direct lookup (env_id might be the dispatch_id)
        view = await self._state_store.get_dispatch(env_id)
        if view is not None:
            return view

        # Scan recent dispatches for matching env_id in provision results
        import json

        from tanren_core.store.views import DispatchListFilter

        dispatches = await self._state_store.query_dispatches(DispatchListFilter(limit=100))
        for d in dispatches:
            steps = await self._state_store.get_steps_for_dispatch(d.dispatch_id)
            for step in steps:
                if not step.result_json:
                    continue
                try:
                    result_data = json.loads(step.result_json)
                    handle = result_data.get("handle", {})
                    if handle.get("env_id") == env_id:
                        return d
                except json.JSONDecodeError, TypeError:
                    continue
        return None
