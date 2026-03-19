"""Postgres-backed event emitter for structured observability."""

from __future__ import annotations

import json
import logging

import asyncpg

from tanren_core.adapters.events import Event

logger = logging.getLogger(__name__)


class PostgresEventEmitter:
    """Writes events to a Postgres database via a shared pool.

    Satisfies the EventEmitter protocol via structural typing.
    The pool is owned externally; close() is a no-op.
    """

    def __init__(self, pool: asyncpg.Pool) -> None:
        """Initialize with an existing asyncpg pool."""
        self._pool = pool

    async def emit(self, event: Event) -> None:
        """Persist an event to the Postgres database."""
        event_type = type(event).__name__
        payload = json.dumps(event.model_dump(mode="json"))
        await self._pool.execute(
            "INSERT INTO events (timestamp, workflow_id, event_type, payload) "
            "VALUES ($1, $2, $3, $4)",
            event.timestamp,
            event.workflow_id,
            event_type,
            payload,
        )

    async def close(self) -> None:
        """No-op — pool is owned externally."""
