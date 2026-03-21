"""Dispatch endpoints — accept, query, and cancel dispatch requests."""

from __future__ import annotations

from typing import Annotated

from fastapi import APIRouter, Depends, Request
from fastapi import Path as PathParam

from tanren_api.dependencies import (
    get_api_store,
    get_emitter,
    get_event_store,
    get_execution_env,
    get_job_queue,
    get_state_store,
)
from tanren_api.models import (
    DispatchAccepted,
    DispatchCancelled,
    DispatchDetail,
    DispatchRequest,
)
from tanren_api.services import DispatchService, DispatchServiceV2

router = APIRouter(tags=["dispatch"])


def _dispatch_service(request: Request) -> DispatchService | DispatchServiceV2:
    """Build the appropriate dispatch service based on available infrastructure.

    Returns V2 (queue-based) when the event-sourced store is available,
    otherwise falls back to V1 (in-process).
    """
    event_store = get_event_store(request)
    if event_store is not None:
        return DispatchServiceV2(
            event_store=event_store,
            job_queue=get_job_queue(request),  # type: ignore[arg-type]
            state_store=get_state_store(request),  # type: ignore[arg-type]
        )
    return DispatchService(
        store=get_api_store(request),
        config=request.app.state.config,
        emitter=get_emitter(request),
        execution_env=get_execution_env(request),
    )


@router.post("/dispatch")
async def create_dispatch(
    body: DispatchRequest,
    service: Annotated[DispatchService | DispatchServiceV2, Depends(_dispatch_service)],
) -> DispatchAccepted:
    """Accept a new dispatch request.

    Returns:
        DispatchAccepted: Accepted response with workflow ID.
    """
    return await service.create(body)


@router.get("/dispatch/{dispatch_id}")
async def get_dispatch(
    dispatch_id: Annotated[str, PathParam(description="Workflow ID")],
    service: Annotated[DispatchService | DispatchServiceV2, Depends(_dispatch_service)],
) -> DispatchDetail:
    """Query dispatch status by workflow ID.

    Returns:
        DispatchDetail: Dispatch details including current status.
    """
    return await service.get(dispatch_id)


@router.delete("/dispatch/{dispatch_id}")
async def cancel_dispatch(
    dispatch_id: Annotated[str, PathParam(description="Workflow ID")],
    service: Annotated[DispatchService | DispatchServiceV2, Depends(_dispatch_service)],
) -> DispatchCancelled:
    """Cancel a pending dispatch.

    Returns:
        DispatchCancelled: Confirmation of the cancelled dispatch.
    """
    return await service.cancel(dispatch_id)
