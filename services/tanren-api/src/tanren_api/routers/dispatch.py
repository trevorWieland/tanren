"""Dispatch endpoints — accept and query dispatch requests."""
# ruff: noqa: DOC501 — all endpoints are stubs that raise NotImplementedAPIError

from typing import Annotated

from fastapi import APIRouter, Path

from tanren_api.errors import NotImplementedAPIError
from tanren_api.models import DispatchAccepted, DispatchRequest

router = APIRouter(tags=["dispatch"])


@router.post("/dispatch")
async def create_dispatch(body: DispatchRequest) -> DispatchAccepted:
    """Accept a new dispatch request."""
    raise NotImplementedAPIError(detail="Dispatch creation not yet implemented")


@router.get("/dispatch/{dispatch_id}")
async def get_dispatch(
    dispatch_id: Annotated[str, Path(description="Workflow ID")],
) -> dict:
    """Query dispatch status by workflow ID."""
    raise NotImplementedAPIError(detail="Dispatch query not yet implemented")


@router.delete("/dispatch/{dispatch_id}")
async def cancel_dispatch(
    dispatch_id: Annotated[str, Path(description="Workflow ID")],
) -> dict:
    """Cancel a pending dispatch."""
    raise NotImplementedAPIError(detail="Dispatch cancellation not yet implemented")
