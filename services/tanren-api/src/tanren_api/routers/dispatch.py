"""Dispatch endpoints — accept, query, and cancel dispatch requests."""

from __future__ import annotations

from typing import Annotated

from fastapi import APIRouter, Depends
from fastapi import Path as PathParam

from tanren_api.dependencies import get_dispatch_service
from tanren_api.models import (
    DispatchAccepted,
    DispatchCancelled,
    DispatchDetail,
    DispatchRequest,
)
from tanren_api.services import DispatchService

router = APIRouter(tags=["dispatch"])


@router.post("/dispatch")
async def create_dispatch(
    body: DispatchRequest,
    service: Annotated[DispatchService, Depends(get_dispatch_service)],
) -> DispatchAccepted:
    """Accept a new dispatch request.

    Returns:
        DispatchAccepted: Accepted response with workflow ID.
    """
    return await service.create(body)


@router.get("/dispatch/{dispatch_id}")
async def get_dispatch(
    dispatch_id: Annotated[str, PathParam(description="Workflow ID")],
    service: Annotated[DispatchService, Depends(get_dispatch_service)],
) -> DispatchDetail:
    """Query dispatch status by workflow ID.

    Returns:
        DispatchDetail: Dispatch details including current status.
    """
    return await service.get(dispatch_id)


@router.delete("/dispatch/{dispatch_id}")
async def cancel_dispatch(
    dispatch_id: Annotated[str, PathParam(description="Workflow ID")],
    service: Annotated[DispatchService, Depends(get_dispatch_service)],
) -> DispatchCancelled:
    """Cancel a pending dispatch.

    Returns:
        DispatchCancelled: Confirmation of the cancelled dispatch.
    """
    return await service.cancel(dispatch_id)
