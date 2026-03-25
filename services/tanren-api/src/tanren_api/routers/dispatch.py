"""Dispatch endpoints — accept, query, and cancel dispatch requests."""

from __future__ import annotations

from typing import Annotated

from fastapi import APIRouter, Depends
from fastapi import Path as PathParam

from tanren_api.dependencies import get_event_store, get_job_queue, get_state_store
from tanren_api.models import (
    DispatchAccepted,
    DispatchCancelled,
    DispatchDetail,
    DispatchRequest,
)
from tanren_api.services.dispatch import DispatchService
from tanren_core.store.protocols import EventStore, JobQueue, StateStore

router = APIRouter(tags=["dispatch"])


@router.post("/dispatch")
async def create_dispatch(
    body: DispatchRequest,
    event_store: Annotated[EventStore, Depends(get_event_store)],
    job_queue: Annotated[JobQueue, Depends(get_job_queue)],
    state_store: Annotated[StateStore, Depends(get_state_store)],
) -> DispatchAccepted:
    """Accept a new dispatch request.

    Returns:
        DispatchAccepted: Accepted response with workflow ID.
    """
    service = DispatchService(event_store=event_store, job_queue=job_queue, state_store=state_store)
    return await service.create(body)


@router.get("/dispatch/{dispatch_id}")
async def get_dispatch(
    dispatch_id: Annotated[str, PathParam(description="Workflow ID")],
    event_store: Annotated[EventStore, Depends(get_event_store)],
    job_queue: Annotated[JobQueue, Depends(get_job_queue)],
    state_store: Annotated[StateStore, Depends(get_state_store)],
) -> DispatchDetail:
    """Query dispatch status by workflow ID.

    Returns:
        DispatchDetail: Dispatch details including current status.
    """
    service = DispatchService(event_store=event_store, job_queue=job_queue, state_store=state_store)
    return await service.get(dispatch_id)


@router.delete("/dispatch/{dispatch_id}")
async def cancel_dispatch(
    dispatch_id: Annotated[str, PathParam(description="Workflow ID")],
    event_store: Annotated[EventStore, Depends(get_event_store)],
    job_queue: Annotated[JobQueue, Depends(get_job_queue)],
    state_store: Annotated[StateStore, Depends(get_state_store)],
) -> DispatchCancelled:
    """Cancel a pending dispatch.

    Returns:
        DispatchCancelled: Confirmation of the cancelled dispatch.
    """
    service = DispatchService(event_store=event_store, job_queue=job_queue, state_store=state_store)
    return await service.cancel(dispatch_id)
