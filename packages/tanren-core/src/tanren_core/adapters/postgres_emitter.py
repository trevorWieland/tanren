"""Postgres-backed event emitter for structured observability."""

from __future__ import annotations

import json
import logging
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import asyncpg

    from tanren_core.adapters.events import Event

logger = logging.getLogger(__name__)

# asyncpg's built-in JSONB codec calls json.dumps() internally, so we must
# pass a Python dict — not a pre-serialised string — to avoid double-encoding.
# The codec is registered per-connection by default in asyncpg ≥ 0.14.
_JSONB_OID = 3802


class PostgresEventEmitter:
    """Writes events to a Postgres database via a shared pool.

    Satisfies the EventEmitter protocol via structural typing.
    The pool is owned externally; close() is a no-op.
    """

    def __init__(self, pool: asyncpg.Pool) -> None:
        """Initialize with an existing asyncpg pool."""
        self._pool = pool

    async def emit(self, event: Event) -> None:
        """Persist an event to the Postgres database.

        Passes the payload as a JSON string and casts to jsonb in SQL
        to avoid double-encoding by asyncpg's built-in JSONB codec.
        """
        event_type = type(event).__name__
        payload_str = json.dumps(event.model_dump(mode="json"))
        await self._pool.execute(
            "INSERT INTO events (timestamp, workflow_id, event_type, payload) "
            "VALUES ($1, $2, $3, $4::jsonb)",
            event.timestamp,
            event.workflow_id,
            event_type,
            payload_str,
        )

    async def close(self) -> None:
        """No-op — pool is owned externally."""
