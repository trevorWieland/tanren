"""Run service — step-by-step provision/execute/teardown via job queue."""

from __future__ import annotations

import logging
import time
import uuid
from datetime import UTC, datetime

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
from tanren_core.schemas import Cli, Dispatch, Outcome, Phase
from tanren_core.store.enums import (
    DispatchMode,
    DispatchStatus,
    StepStatus,
    StepType,
    cli_to_lane,
)
from tanren_core.store.events import DispatchCreated
from tanren_core.store.payloads import (
    ExecuteStepPayload,
    ProvisionStepPayload,
    TeardownStepPayload,
)
from tanren_core.store.protocols import EventStore, JobQueue, StateStore
from tanren_core.store.views import DispatchView

logger = logging.getLogger(__name__)


def _now() -> str:
    return datetime.now(UTC).isoformat().replace("+00:00", "Z")


class RunService:
    """Stateless run service — enqueues steps, reads from state store."""

    def __init__(
        self,
        *,
        event_store: EventStore,
        job_queue: JobQueue,
        state_store: StateStore,
    ) -> None:
        """Initialize with store dependencies only."""
        self._event_store = event_store
        self._job_queue = job_queue
        self._state_store = state_store

    async def provision(self, body: ProvisionRequest) -> RunEnvironment:
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

        lane = cli_to_lane(dispatch.cli)

        # Create dispatch in MANUAL mode
        await self._state_store.create_dispatch_projection(
            dispatch_id=workflow_id,
            mode=DispatchMode.MANUAL,
            lane=lane,
            preserve_on_failure=True,
            dispatch_json=dispatch.model_dump_json(),
        )

        await self._event_store.append(
            DispatchCreated(
                timestamp=_now(),
                workflow_id=workflow_id,
                dispatch=dispatch,
                mode=DispatchMode.MANUAL,
                lane=lane,
            )
        )

        step_id = uuid.uuid4().hex
        payload = ProvisionStepPayload(dispatch=dispatch)
        await self._job_queue.enqueue_step(
            step_id=step_id,
            dispatch_id=workflow_id,
            step_type="provision",
            step_sequence=0,
            lane=None,
            payload_json=payload.model_dump_json(),
        )

        return RunEnvironment(
            env_id=workflow_id,
            dispatch_id=workflow_id,
            status=RunEnvironmentStatus.PROVISIONING,
        )

    async def execute(self, env_id: str, body: ExecuteRequest) -> RunExecuteAccepted:
        """Enqueue an execute step for a provisioned environment.

        Returns:
            RunExecuteAccepted with dispatch_id.
        """
        # Find the dispatch by scanning for the provisioned environment
        # In the queue model, env_id maps to a dispatch_id
        dispatch_view = await self._find_dispatch_for_env(env_id)
        if dispatch_view is None:
            raise NotFoundError(f"Environment {env_id} not found")
        if dispatch_view.status == DispatchStatus.CANCELLED:
            raise ServiceError(f"Dispatch {dispatch_view.dispatch_id} is cancelled")

        # Get the provision step's result to extract the handle
        steps = await self._state_store.get_steps_for_dispatch(dispatch_view.dispatch_id)
        provision_step = next(
            (
                s
                for s in steps
                if s.step_type == StepType.PROVISION and s.status == StepStatus.COMPLETED
            ),
            None,
        )
        if provision_step is None or provision_step.result_json is None:
            raise ServiceError("Provision step not completed — cannot execute")

        from tanren_core.store.payloads import ProvisionResult

        prov_result = ProvisionResult.model_validate_json(provision_step.result_json)

        dispatch = dispatch_view.dispatch

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
            cli=body.cli,
            auth=body.auth,
            model=body.model,
            timeout=body.timeout,
            context=body.context,
            gate_cmd=body.gate_cmd,
        )

        # Guard: prevent concurrent execute steps (allow sequential phases)
        if any(
            s.step_type == StepType.EXECUTE and s.status in (StepStatus.PENDING, StepStatus.RUNNING)
            for s in steps
        ):
            raise ServiceError("Execute step already in progress for this environment")

        # Guard: block execute after teardown has been requested
        if any(
            s.step_type == StepType.TEARDOWN
            and s.status in (StepStatus.PENDING, StepStatus.RUNNING, StepStatus.COMPLETED)
            for s in steps
        ):
            raise ServiceError("Cannot execute after teardown")

        lane = cli_to_lane(exec_dispatch.cli)

        # Compute next sequence number for multi-phase support
        max_seq = max((s.step_sequence for s in steps), default=0)
        step_id = uuid.uuid4().hex
        payload = ExecuteStepPayload(dispatch=exec_dispatch, handle=prov_result.handle)
        await self._job_queue.enqueue_step(
            step_id=step_id,
            dispatch_id=dispatch_view.dispatch_id,
            step_type="execute",
            step_sequence=max_seq + 1,
            lane=str(lane),
            payload_json=payload.model_dump_json(),
        )

        return RunExecuteAccepted(
            env_id=dispatch_view.dispatch_id, dispatch_id=dispatch_view.dispatch_id
        )

    async def teardown(self, env_id: str) -> RunTeardownAccepted:
        """Enqueue a teardown step.

        Returns:
            RunTeardownAccepted with dispatch_id.
        """
        dispatch_view = await self._find_dispatch_for_env(env_id)
        if dispatch_view is None:
            raise NotFoundError(f"Environment {env_id} not found")

        steps = await self._state_store.get_steps_for_dispatch(dispatch_view.dispatch_id)
        provision_step = next(
            (
                s
                for s in steps
                if s.step_type == StepType.PROVISION and s.status == StepStatus.COMPLETED
            ),
            None,
        )
        if provision_step is None or provision_step.result_json is None:
            raise ServiceError("Provision step not completed — cannot teardown")

        # Guard: block teardown while execute is still active
        if any(
            s.step_type == StepType.EXECUTE and s.status in (StepStatus.PENDING, StepStatus.RUNNING)
            for s in steps
        ):
            raise ServiceError("Cannot teardown while execute is in progress")

        # Guard: prevent concurrent teardown steps (allow retry after failure)
        if any(
            s.step_type == StepType.TEARDOWN
            and s.status in (StepStatus.PENDING, StepStatus.RUNNING, StepStatus.COMPLETED)
            for s in steps
        ):
            raise ServiceError("Teardown already enqueued or completed for this environment")

        from tanren_core.store.payloads import ProvisionResult

        prov_result = ProvisionResult.model_validate_json(provision_step.result_json)

        dispatch = dispatch_view.dispatch
        max_seq = max((s.step_sequence for s in steps), default=0)
        step_id = uuid.uuid4().hex
        payload = TeardownStepPayload(dispatch=dispatch, handle=prov_result.handle)
        await self._job_queue.enqueue_step(
            step_id=step_id,
            dispatch_id=dispatch_view.dispatch_id,
            step_type="teardown",
            step_sequence=max_seq + 1,
            lane=None,
            payload_json=payload.model_dump_json(),
        )

        return RunTeardownAccepted(
            env_id=dispatch_view.dispatch_id, dispatch_id=dispatch_view.dispatch_id
        )

    async def full(self, body: RunFullRequest) -> DispatchAccepted:
        """Enqueue a full dispatch lifecycle (auto-chained provision → execute → teardown).

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
        )

    async def status(self, env_id: str) -> RunStatus:
        """Check status of a run environment.

        Returns:
            RunStatus with current state.
        """
        dispatch_view = await self._find_dispatch_for_env(env_id)
        if dispatch_view is None:
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

        return RunStatus(
            env_id=env_id,
            dispatch_id=dispatch_view.dispatch_id,
            status=env_status,
            outcome=dispatch_view.outcome,
        )

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
