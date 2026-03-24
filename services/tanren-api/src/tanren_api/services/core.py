"""Simple services — health, config, events."""

from __future__ import annotations

import importlib.metadata
import logging
import time
from typing import TYPE_CHECKING

from tanren_api.models import ConfigResponse, HealthResponse, PaginatedEvents, ReadinessResponse

if TYPE_CHECKING:
    from pydantic import TypeAdapter

    from tanren_api.models import EventPayload
    from tanren_api.settings import APISettings
    from tanren_core.store.protocols import EventStore, StateStore

logger = logging.getLogger(__name__)

_start_time = time.monotonic()


class HealthService:
    """Service for health and readiness checks."""

    async def health(self) -> HealthResponse:
        """Return service health and version info.

        Returns:
            HealthResponse: Service health status and version.
        """
        return HealthResponse(
            status="ok",
            version="0.1.0",
            uptime_seconds=round(time.monotonic() - _start_time, 2),
        )

    async def readiness(self) -> ReadinessResponse:
        """Return readiness probe response.

        Returns:
            ReadinessResponse: Service readiness status.
        """
        return ReadinessResponse(status="ready")


class ConfigService:
    """Service for non-secret config projection (V2 fields)."""

    def __init__(self, settings: APISettings, state_store: StateStore) -> None:
        """Initialize with API settings and state store."""
        self._settings = settings
        self._state_store = state_store

    async def get(self) -> ConfigResponse:
        """Return V2 config fields.

        Returns:
            ConfigResponse: Non-secret configuration fields.
        """
        db_url = self._settings.db_url
        db_backend = "postgres" if db_url.startswith(("postgresql", "postgres")) else "sqlite"

        store_connected = True
        try:
            from tanren_core.store.views import DispatchListFilter

            await self._state_store.query_dispatches(DispatchListFilter(limit=1))
        except Exception:
            store_connected = False

        try:
            version = importlib.metadata.version("tanren-api")
        except importlib.metadata.PackageNotFoundError:
            version = "unknown"

        return ConfigResponse(
            db_backend=db_backend,
            store_connected=store_connected,
            worker_lanes={"impl": 1, "audit": 1, "gate": 3, "provision": 10},
            remote_enabled=False,
            version=version,
        )


class EventsService:
    """Service for querying structured events via EventStore."""

    def __init__(self, event_store: EventStore) -> None:
        """Initialize with the unified event store."""
        self._event_store = event_store

    async def query(
        self,
        *,
        workflow_id: str | None = None,
        event_type: str | None = None,
        limit: int = 50,
        offset: int = 0,
    ) -> PaginatedEvents:
        """Query structured events with optional filters.

        Returns:
            PaginatedEvents: Paginated list of matching events.
        """
        result = await self._event_store.query_events(
            dispatch_id=workflow_id,
            event_type=event_type,
            limit=limit,
            offset=offset,
        )

        events: list[EventPayload] = []
        skipped = 0
        adapter: TypeAdapter[EventPayload] = _get_event_adapter()
        for row in result.events:
            try:
                event = adapter.validate_python(row.payload)
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
            skipped=skipped + result.skipped,
        )


def _get_event_adapter() -> TypeAdapter[EventPayload]:
    """Lazy-initialize the event TypeAdapter (avoids import-time cost).

    Returns:
        TypeAdapter[EventPayload]: Cached Pydantic type adapter for event payloads.
    """
    from pydantic import TypeAdapter as TA

    from tanren_api.models import EventPayload as EP

    global _event_adapter
    if _event_adapter is None:
        _event_adapter = TA(EP)
    return _event_adapter


_event_adapter: TypeAdapter[EventPayload] | None = None
