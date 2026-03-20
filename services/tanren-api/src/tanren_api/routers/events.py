"""Events endpoint — query structured events."""

from __future__ import annotations

from typing import Annotated

from fastapi import APIRouter, Depends, Query, Request

from tanren_api.dependencies import get_event_reader, get_settings
from tanren_api.models import PaginatedEvents
from tanren_api.services import EventsService
from tanren_api.settings import APISettings
from tanren_core.adapters.event_reader import EventReader

router = APIRouter(tags=["events"])


@router.get("/events")
async def list_events(
    request: Request,
    settings: Annotated[APISettings, Depends(get_settings)],
    event_reader: Annotated[EventReader | None, Depends(get_event_reader)],
    workflow_id: Annotated[str | None, Query(description="Filter by workflow ID")] = None,
    event_type: Annotated[str | None, Query(description="Filter by event type")] = None,
    limit: Annotated[int, Query(ge=1, le=100, description="Page size")] = 50,
    offset: Annotated[int, Query(ge=0, description="Pagination offset")] = 0,
) -> PaginatedEvents:
    """Query structured events with optional filters.

    Returns:
        PaginatedEvents: Paginated list of matching events.
    """
    config = getattr(request.app.state, "config", None)
    return await EventsService(settings, config, event_reader=event_reader).query(
        workflow_id=workflow_id,
        event_type=event_type,
        limit=limit,
        offset=offset,
    )
