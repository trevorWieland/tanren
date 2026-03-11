"""SQLite-backed event emitter for structured observability."""

from __future__ import annotations

import json
import logging
from pathlib import Path

import aiosqlite

from worker_manager.adapters.events import Event

logger = logging.getLogger(__name__)

_SCHEMA = """\
CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    workflow_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    payload TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_events_workflow ON events(workflow_id);
CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);
CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events(timestamp);
"""


class SqliteEventEmitter:
    """Writes events to a SQLite database.

    Lazily opens the connection on first emit().
    """

    def __init__(self, db_path: str | Path) -> None:
        self._db_path = Path(db_path)
        self._conn: aiosqlite.Connection | None = None

    async def _ensure_conn(self) -> aiosqlite.Connection:
        if self._conn is None:
            self._db_path.parent.mkdir(parents=True, exist_ok=True)
            self._conn = await aiosqlite.connect(str(self._db_path))
            await self._conn.executescript(_SCHEMA)
        return self._conn

    async def emit(self, event: Event) -> None:
        conn = await self._ensure_conn()
        event_type = type(event).__name__
        payload = json.dumps(event.model_dump(mode="json"))
        await conn.execute(
            "INSERT INTO events (timestamp, workflow_id, event_type, payload) VALUES (?, ?, ?, ?)",
            (event.timestamp, event.workflow_id, event_type, payload),
        )
        await conn.commit()

    async def close(self) -> None:
        if self._conn is not None:
            await self._conn.close()
            self._conn = None
