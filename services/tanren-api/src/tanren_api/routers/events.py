"""Events endpoint — query structured events."""
# ruff: noqa: DOC201

from __future__ import annotations

import logging
from typing import Annotated

from fastapi import APIRouter, Depends, Query, Request
from pydantic import TypeAdapter

from tanren_api.dependencies import get_settings
from tanren_api.models import EventPayload, PaginatedEvents
from tanren_api.settings import APISettings
from tanren_core.adapters.event_reader import query_events

logger = logging.getLogger(__name__)

router = APIRouter(tags=["events"])

_event_adapter = TypeAdapter(EventPayload)


@router.get("/events")
async def list_events(
    request: Request,
    settings: Annotated[APISettings, Depends(get_settings)],
    workflow_id: Annotated[str | None, Query(description="Filter by workflow ID")] = None,
    event_type: Annotated[str | None, Query(description="Filter by event type")] = None,
    limit: Annotated[int, Query(ge=1, le=100, description="Page size")] = 50,
    offset: Annotated[int, Query(ge=0, description="Pagination offset")] = 0,
) -> PaginatedEvents:
    """Query structured events with optional filters."""
    # Determine DB path — settings first, then config fallback
    db_path = settings.events_db
    if db_path is None:
        config = getattr(request.app.state, "config", None)
        if config is not None:
            db_path = config.events_db

    if not db_path:
        return PaginatedEvents(events=[], total=0, limit=limit, offset=offset)

    result = await query_events(
        db_path,
        workflow_id=workflow_id,
        event_type=event_type,
        limit=limit,
        offset=offset,
    )

    # Parse payloads into typed events
    events = []
    skipped = 0
    for row in result.events:
        try:
            event = _event_adapter.validate_python(row.payload)
            events.append(event)
        except Exception:
            skipped += 1
            logger.warning(
                "Skipping unparseable event %d: %s", row.id, row.event_type, exc_info=True
            )

    return PaginatedEvents(
        events=events,
        total=result.total,
        limit=limit,
        offset=offset,
        skipped=skipped,
    )
