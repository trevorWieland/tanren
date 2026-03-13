"""Events endpoint — query structured events."""

from typing import Annotated

from fastapi import APIRouter, Query

from tanren_api.errors import NotImplementedError
from tanren_api.models import PaginatedEvents

router = APIRouter(tags=["events"])


@router.get("/events")
async def list_events(
    workflow_id: Annotated[str | None, Query(description="Filter by workflow ID")] = None,
    event_type: Annotated[str | None, Query(description="Filter by event type")] = None,
    limit: Annotated[int, Query(ge=1, le=100, description="Page size")] = 50,
    offset: Annotated[int, Query(ge=0, description="Pagination offset")] = 0,
) -> PaginatedEvents:
    """Query structured events with optional filters."""
    raise NotImplementedError(detail="Event querying not yet implemented")
