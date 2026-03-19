"""Simple services — health, config, events."""
# ruff: noqa: DOC201

from __future__ import annotations

import logging
import time
from typing import TYPE_CHECKING

from tanren_api.models import ConfigResponse, HealthResponse, PaginatedEvents, ReadinessResponse

if TYPE_CHECKING:
    from pydantic import TypeAdapter

    from tanren_api.models import EventPayload
    from tanren_api.settings import APISettings
    from tanren_core.adapters.event_reader import EventReader
    from tanren_core.config import Config

logger = logging.getLogger(__name__)

_start_time = time.monotonic()


class HealthService:
    """Service for health and readiness checks."""

    async def health(self) -> HealthResponse:
        """Return service health and version info."""
        return HealthResponse(
            status="ok",
            version="0.1.0",
            uptime_seconds=round(time.monotonic() - _start_time, 2),
        )

    async def readiness(self) -> ReadinessResponse:
        """Return readiness probe response."""
        return ReadinessResponse(status="ready")


class ConfigService:
    """Service for non-secret config projection."""

    def __init__(self, config: Config) -> None:
        """Initialize with core config."""
        self._config = config

    async def get(self) -> ConfigResponse:
        """Return non-secret config fields."""
        c = self._config
        return ConfigResponse(
            ipc_dir=c.ipc_dir,
            github_dir=c.github_dir,
            poll_interval=c.poll_interval,
            heartbeat_interval=c.heartbeat_interval,
            max_opencode=c.max_opencode,
            max_codex=c.max_codex,
            max_gate=c.max_gate,
            events_enabled=c.events_db is not None,
            remote_enabled=c.remote_config_path is not None,
        )


class EventsService:
    """Service for querying structured events."""

    def __init__(
        self,
        settings: APISettings,
        config: Config | None = None,
        event_reader: EventReader | None = None,
    ) -> None:
        """Initialize with settings, optional config, and optional event reader."""
        self._settings = settings
        self._config = config
        self._event_reader = event_reader

    async def query(
        self,
        *,
        workflow_id: str | None = None,
        event_type: str | None = None,
        limit: int = 50,
        offset: int = 0,
    ) -> PaginatedEvents:
        """Query structured events with optional filters."""
        if self._event_reader is not None:
            result = await self._event_reader.query_events(
                workflow_id=workflow_id,
                event_type=event_type,
                limit=limit,
                offset=offset,
            )
        else:
            from tanren_core.adapters.event_reader import query_events  # noqa: PLC0415

            db_path = self._settings.events_db
            if db_path is None and self._config is not None:
                db_path = self._config.events_db

            if not db_path:
                return PaginatedEvents(events=[], total=0, limit=limit, offset=offset)

            result = await query_events(
                db_path,
                workflow_id=workflow_id,
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
    """Lazy-initialize the event TypeAdapter (avoids import-time cost)."""
    from pydantic import TypeAdapter as TA  # noqa: PLC0415

    from tanren_api.models import EventPayload as EP  # noqa: PLC0415

    global _event_adapter
    if _event_adapter is None:
        _event_adapter = TA(EP)
    return _event_adapter


_event_adapter: TypeAdapter[EventPayload] | None = None
