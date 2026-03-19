"""Postgres-backed event reader for querying structured events."""

from __future__ import annotations

import logging

import asyncpg

from tanren_core.adapters.event_reader import EventQueryResult, EventRow

logger = logging.getLogger(__name__)


class PostgresEventReader:
    """Reads events from a Postgres database via a shared pool.

    Satisfies the EventReader protocol via structural typing.
    """

    def __init__(self, pool: asyncpg.Pool) -> None:
        """Initialize with an existing asyncpg pool."""
        self._pool = pool

    async def query_events(
        self,
        *,
        workflow_id: str | None = None,
        event_type: str | None = None,
        limit: int = 50,
        offset: int = 0,
    ) -> EventQueryResult:
        """Query events with optional filters and pagination.

        Returns:
            EventQueryResult with matching events and total count.
        """
        where_clauses: list[str] = []
        params: list[str | int] = []
        idx = 1

        if workflow_id is not None:
            where_clauses.append(f"workflow_id = ${idx}")
            params.append(workflow_id)
            idx += 1
        if event_type is not None:
            where_clauses.append(f"event_type = ${idx}")
            params.append(event_type)
            idx += 1

        where_sql = ""
        if where_clauses:
            where_sql = " WHERE " + " AND ".join(where_clauses)

        # Total count
        count_sql = f"SELECT COUNT(*) FROM events{where_sql}"
        total = await self._pool.fetchval(count_sql, *params)

        # Fetch page
        select_sql = (
            f"SELECT id, timestamp, workflow_id, event_type, payload "
            f"FROM events{where_sql} ORDER BY timestamp DESC "
            f"LIMIT ${idx} OFFSET ${idx + 1}"
        )
        rows = await self._pool.fetch(select_sql, *params, limit, offset)

        events: list[EventRow] = []
        skipped = 0
        for r in rows:
            payload = r["payload"]
            # asyncpg returns JSONB as dicts/lists directly
            if not isinstance(payload, dict):
                skipped += 1
                logger.warning("Skipping event %d: unexpected payload type", r["id"])
                continue
            events.append(
                EventRow(
                    id=r["id"],
                    timestamp=r["timestamp"],
                    workflow_id=r["workflow_id"],
                    event_type=r["event_type"],
                    payload=payload,
                )
            )

        return EventQueryResult(events=events, total=total or 0, skipped=skipped)
