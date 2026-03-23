"""Dispatch service — enqueue-only, reads from StateStore."""

from __future__ import annotations

import logging

from tanren_api.errors import ConflictError, NotFoundError
from tanren_api.models import (
    DispatchAccepted,
    DispatchCancelled,
    DispatchDetail,
    DispatchRequest,
    DispatchRunStatus,
)
from tanren_api.services.dispatch_lifecycle import create_dispatch_from_request
from tanren_core.store.enums import DispatchStatus
from tanren_core.store.protocols import EventStore, JobQueue, StateStore

logger = logging.getLogger(__name__)


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
        return await create_dispatch_from_request(
            body=body,
            event_store=self._event_store,
            job_queue=self._job_queue,
            state_store=self._state_store,
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
        # Not atomic with status update, but safe: if this fails, the
        # CANCELLED status + worker auto-chain guards prevent new steps
        # from being chained.
        await self._job_queue.cancel_pending_steps(dispatch_id)

        return DispatchCancelled(dispatch_id=dispatch_id)
