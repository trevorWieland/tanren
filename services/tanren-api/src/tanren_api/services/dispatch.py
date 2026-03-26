"""Dispatch service — enqueue-only, reads from StateStore."""

from __future__ import annotations

import logging
import uuid
from typing import TYPE_CHECKING

from tanren_api.errors import ConflictError, NotFoundError
from tanren_api.models import (
    DispatchAccepted,
    DispatchCancelled,
    DispatchDetail,
    DispatchRequest,
    DispatchRunStatus,
)
from tanren_api.services.dispatch_lifecycle import create_dispatch_from_request
from tanren_core.store.enums import DispatchMode, DispatchStatus, StepStatus, StepType
from tanren_core.store.protocols import EventStore, JobQueue, StateStore
from tanren_core.store.views import DispatchView

if TYPE_CHECKING:
    from tanren_core.worker_config import WorkerConfig

logger = logging.getLogger(__name__)


class DispatchService:
    """Stateless dispatch service — enqueues to job queue, reads from state store."""

    def __init__(
        self,
        *,
        event_store: EventStore,
        job_queue: JobQueue,
        state_store: StateStore,
        config: WorkerConfig | None = None,
    ) -> None:
        """Initialize with store dependencies only — no filesystem access."""
        self._event_store = event_store
        self._job_queue = job_queue
        self._state_store = state_store
        self._config = config

    async def create(self, body: DispatchRequest, user_id: str = "") -> DispatchAccepted:
        """Accept a new dispatch request by enqueuing a provision step.

        The caller must provide ``resolved_profile`` in the request body.
        The API validates the schema but does not resolve profiles from
        the filesystem.

        Returns:
            DispatchAccepted with the workflow ID.
        """
        return await create_dispatch_from_request(
            body=body,
            event_store=self._event_store,
            job_queue=self._job_queue,
            state_store=self._state_store,
            config=self._config,
            user_id=user_id,
        )

    async def get(self, dispatch_id: str) -> DispatchDetail:
        """Query dispatch status from the state store.

        Returns:
            DispatchDetail with current status from projections.

        Raises:
            NotFoundError: If not found.
        """
        view = await self._state_store.get_dispatch(dispatch_id)
        if view is None:
            raise NotFoundError(f"Dispatch {dispatch_id} not found")

        d = view.dispatch
        return DispatchDetail(
            workflow_id=d.workflow_id,
            phase=d.phase,
            project=d.project,
            spec_folder=d.spec_folder,
            branch=d.branch,
            cli=d.cli,
            auth=d.auth,
            model=d.model,
            timeout=d.timeout,
            environment_profile=d.environment_profile,
            context=d.context,
            gate_cmd=d.gate_cmd,
            status=DispatchRunStatus(str(view.status)),
            outcome=view.outcome,
            created_at=view.created_at,
            started_at=None,
            completed_at=None,
        )

    async def cancel(self, dispatch_id: str) -> DispatchCancelled:
        """Cancel a dispatch by updating the state store.

        Returns:
            DispatchCancelled confirmation.

        Raises:
            NotFoundError: If not found.
            ConflictError: If already terminal.
        """
        view = await self._state_store.get_dispatch(dispatch_id)
        if view is None:
            raise NotFoundError(f"Dispatch {dispatch_id} not found")

        if view.status in (
            DispatchStatus.COMPLETED,
            DispatchStatus.FAILED,
            DispatchStatus.CANCELLED,
        ):
            raise ConflictError(f"Dispatch {dispatch_id} is already {view.status}")

        await self._state_store.update_dispatch_status(dispatch_id, DispatchStatus.CANCELLED)

        # Cancel pending forward-progress steps (teardowns preserved).
        await self._job_queue.cancel_pending_steps(dispatch_id)

        # If provision already completed, enqueue cleanup teardown so
        # remote VMs/workspaces are released
        await self._enqueue_cancel_teardown(dispatch_id, view)

        return DispatchCancelled(dispatch_id=dispatch_id)

    async def _enqueue_cancel_teardown(self, dispatch_id: str, view: DispatchView) -> None:
        """Enqueue cleanup teardown if provision completed but no teardown exists."""
        from tanren_core.store.payloads import ProvisionResult, TeardownStepPayload

        steps = await self._state_store.get_steps_for_dispatch(dispatch_id)
        prov = next(
            (
                s
                for s in steps
                if s.step_type == StepType.PROVISION
                and s.status == StepStatus.COMPLETED
                and s.result_json
            ),
            None,
        )
        if prov is None or any(s.step_type == StepType.TEARDOWN for s in steps):
            return
        # For AUTO dispatches, skip teardown enqueue while execute is active —
        # the worker's auto-chain will handle teardown after execute completes.
        # For MANUAL dispatches, always enqueue teardown since auto-chain won't run.
        if view.mode == DispatchMode.AUTO and any(
            s.step_type == StepType.EXECUTE and s.status in (StepStatus.PENDING, StepStatus.RUNNING)
            for s in steps
        ):
            return
        assert prov.result_json is not None
        prov_result = ProvisionResult.model_validate_json(prov.result_json)
        max_seq = max((s.step_sequence for s in steps), default=0)
        await self._job_queue.enqueue_step(
            step_id=uuid.uuid4().hex,
            dispatch_id=dispatch_id,
            step_type="teardown",
            step_sequence=max_seq + 1,
            lane=None,
            payload_json=TeardownStepPayload(
                dispatch=view.dispatch, handle=prov_result.handle
            ).model_dump_json(),
        )
