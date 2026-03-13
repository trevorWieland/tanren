"""Read-only event query function for the SQLite events database."""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path

import aiosqlite


@dataclass(frozen=True)
class EventRow:
    """Single event row from the database."""

    id: int
    timestamp: str
    workflow_id: str
    event_type: str
    payload: dict[str, object]


@dataclass(frozen=True)
class EventQueryResult:
    """Result of an event query with pagination metadata."""

    events: list[EventRow] = field(default_factory=list)
    total: int = 0


async def query_events(
    db_path: str | Path,
    *,
    workflow_id: str | None = None,
    event_type: str | None = None,
    limit: int = 50,
    offset: int = 0,
) -> EventQueryResult:
    """Query events from the SQLite events database.

    Opens a read-only connection. Builds WHERE clause from filters.
    Returns events ordered by timestamp DESC with pagination.

    Returns:
        EventQueryResult with matching events and total count.
    """
    db_path = Path(db_path)
    if not db_path.exists():
        return EventQueryResult()

    where_clauses: list[str] = []
    params: list[str] = []

    if workflow_id is not None:
        where_clauses.append("workflow_id = ?")
        params.append(workflow_id)
    if event_type is not None:
        where_clauses.append("event_type = ?")
        params.append(event_type)

    where_sql = ""
    if where_clauses:
        where_sql = " WHERE " + " AND ".join(where_clauses)

    async with aiosqlite.connect(f"file:{db_path}?mode=ro", uri=True) as conn:
        # Total count
        count_sql = f"SELECT COUNT(*) FROM events{where_sql}"
        cursor = await conn.execute(count_sql, params)
        row = await cursor.fetchone()
        total = row[0] if row else 0

        # Fetch page
        select_sql = (
            f"SELECT id, timestamp, workflow_id, event_type, payload "
            f"FROM events{where_sql} ORDER BY timestamp DESC LIMIT ? OFFSET ?"
        )
        cursor = await conn.execute(select_sql, [*params, limit, offset])
        rows = await cursor.fetchall()

    events = [
        EventRow(
            id=r[0],
            timestamp=r[1],
            workflow_id=r[2],
            event_type=r[3],
            payload=json.loads(r[4]),
        )
        for r in rows
    ]

    return EventQueryResult(events=events, total=total)
