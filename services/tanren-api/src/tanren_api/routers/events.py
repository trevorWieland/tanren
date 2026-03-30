"""Events endpoint — query structured events."""

from __future__ import annotations

from typing import Annotated

from fastapi import APIRouter, Depends, Query

from tanren_api.auth import require_scope
from tanren_api.dependencies import get_event_store, get_state_store
from tanren_api.models import PaginatedEvents
from tanren_api.scopes import has_scope
from tanren_api.services import EventsService
from tanren_core.store.auth_views import AuthContext
from tanren_core.store.protocols import EventStore, StateStore

router = APIRouter(tags=["events"])


async def _user_entity_ids(auth: AuthContext, state_store: StateStore) -> list[str] | None:
    """Build the entity_ids filter for non-admin users.

    Returns None for admins (no filtering).
    For non-admins, returns their dispatch IDs + their own user/key entity IDs.
    """
    if has_scope(auth.scopes, "admin:*"):
        return None
    from tanren_core.store.views import DispatchListFilter

    dispatches = await state_store.query_dispatches(
        DispatchListFilter(user_id=auth.user.user_id, limit=10000)
    )
    entity_ids = [d.dispatch_id for d in dispatches]
    entity_ids.extend([auth.user.user_id, auth.key.key_id])
    return entity_ids


@router.get("/events")
async def list_events(
    auth: Annotated[AuthContext, Depends(require_scope("events:read"))],
    event_store: Annotated[EventStore, Depends(get_event_store)],
    state_store: Annotated[StateStore, Depends(get_state_store)],
    workflow_id: Annotated[str | None, Query(description="Filter by workflow/entity ID")] = None,
    entity_type: Annotated[
        str | None, Query(description="Filter by entity type (dispatch, user, api_key)")
    ] = None,
    event_type: Annotated[str | None, Query(description="Filter by event type")] = None,
    limit: Annotated[int, Query(ge=1, le=100, description="Page size")] = 50,
    offset: Annotated[int, Query(ge=0, description="Pagination offset")] = 0,
) -> PaginatedEvents:
    """Query structured events with optional filters."""
    entity_ids = await _user_entity_ids(auth, state_store)
    return await EventsService(event_store).query(
        workflow_id=workflow_id,
        entity_ids=entity_ids,
        entity_type=entity_type,
        event_type=event_type,
        limit=limit,
        offset=offset,
    )
