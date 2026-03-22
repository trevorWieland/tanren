"""Dispatch service — enqueue-only, reads from StateStore."""

from __future__ import annotations

import logging
import time
import uuid
from datetime import UTC, datetime

from tanren_api.errors import ConflictError, NotFoundError
from tanren_api.models import (
    DispatchAccepted,
    DispatchCancelled,
    DispatchDetail,
    DispatchRequest,
    DispatchRunStatus,
)
from tanren_core.schemas import Dispatch
from tanren_core.store.enums import DispatchMode, DispatchStatus, cli_to_lane
from tanren_core.store.events import DispatchCreated
from tanren_core.store.payloads import ProvisionStepPayload
from tanren_core.store.protocols import EventStore, JobQueue, StateStore

logger = logging.getLogger(__name__)


def _now() -> str:
    return datetime.now(UTC).isoformat()


class DispatchService:
    """Stateless dispatch service — enqueues to job queue, reads from state store."""

    def __init__(
        self,
        *,
        event_store: EventStore,
        job_queue: JobQueue,
        state_store: StateStore,
    ) -> None:
        """Initialize with store dependencies only — no filesystem access."""
        self._event_store = event_store
        self._job_queue = job_queue
        self._state_store = state_store

    async def create(self, body: DispatchRequest) -> DispatchAccepted:
        """Accept a new dispatch request by enqueuing a provision step.

        The caller must provide ``resolved_profile`` in the request body.
        The API validates the schema but does not resolve profiles from
        the filesystem.

        Returns:
            DispatchAccepted with the workflow ID.
        """
        epoch = time.time_ns()
        issue = body.issue if body.issue != "0" else str(epoch)
        workflow_id = f"wf-{body.project}-{issue}-{epoch}"

        dispatch = Dispatch(
            workflow_id=workflow_id,
            project=body.project,
            phase=body.phase,
            branch=body.branch,
            spec_folder=body.spec_folder,
            cli=body.cli,
            auth=body.auth,
            model=body.model,
            timeout=body.timeout,
            environment_profile=body.resolved_profile.name,
            context=body.context,
            gate_cmd=body.gate_cmd,
            resolved_profile=body.resolved_profile,
            preserve_on_failure=body.preserve_on_failure,
        )

        lane = cli_to_lane(body.cli)

        # 1. Create dispatch projection
        await self._state_store.create_dispatch_projection(
            dispatch_id=workflow_id,
            mode=DispatchMode.AUTO,
            lane=lane,
            preserve_on_failure=dispatch.preserve_on_failure,
            dispatch_json=dispatch.model_dump_json(),
        )

        # 2. Append DispatchCreated event
        await self._event_store.append(
            DispatchCreated(
                timestamp=_now(),
                workflow_id=workflow_id,
                dispatch=dispatch,
                mode=DispatchMode.AUTO,
                lane=lane,
            )
        )

        # 3. Enqueue provision step (worker will auto-chain to execute → teardown)
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

        return DispatchAccepted(dispatch_id=workflow_id)

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

        return DispatchCancelled(dispatch_id=dispatch_id)
