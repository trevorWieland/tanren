"""Events endpoint — query structured events."""

from __future__ import annotations

from typing import Annotated

from fastapi import APIRouter, Depends, Query

from tanren_api.dependencies import get_event_store
from tanren_api.models import PaginatedEvents
from tanren_api.services import EventsService
from tanren_core.store.protocols import EventStore

router = APIRouter(tags=["events"])


@router.get("/events")
async def list_events(
    event_store: Annotated[EventStore, Depends(get_event_store)],
    workflow_id: Annotated[str | None, Query(description="Filter by workflow ID")] = None,
    event_type: Annotated[str | None, Query(description="Filter by event type")] = None,
    limit: Annotated[int, Query(ge=1, le=100, description="Page size")] = 50,
    offset: Annotated[int, Query(ge=0, description="Pagination offset")] = 0,
) -> PaginatedEvents:
    """Query structured events with optional filters."""
    return await EventsService(event_store).query(
        workflow_id=workflow_id,
        event_type=event_type,
        limit=limit,
        offset=offset,
    )
