"""SQLite implementation of EventStore, JobQueue, and StateStore protocols.

A single ``SqliteStore`` instance owns one ``aiosqlite`` connection and
implements all three protocols.  It uses WAL mode for concurrent reads and
``BEGIN IMMEDIATE`` for writes to prevent SQLITE_BUSY under contention.
"""

from __future__ import annotations

import json
import uuid
from collections.abc import AsyncIterator
from contextlib import asynccontextmanager
from datetime import UTC, datetime
from pathlib import Path
from typing import TYPE_CHECKING

import aiosqlite

from tanren_core.adapters.events import Event
from tanren_core.schemas import Dispatch, Outcome
from tanren_core.store.enums import DispatchMode, DispatchStatus, Lane, StepStatus, StepType
from tanren_core.store.events import StepEnqueued
from tanren_core.store.schema import SQLITE_ALL
from tanren_core.store.views import (
    DispatchListFilter,
    DispatchView,
    EventQueryResult,
    EventRow,
    QueuedStep,
    StepView,
)

if TYPE_CHECKING:
    pass


def _now() -> str:
    return datetime.now(UTC).isoformat().replace("+00:00", "Z")


class SqliteStore:
    """Unified SQLite store implementing EventStore, JobQueue, and StateStore."""

    def __init__(self, db_path: str | Path) -> None:
        """Initialise with path to the SQLite database file."""
        self._db_path = Path(db_path)
        self._conn: aiosqlite.Connection | None = None

    async def ensure_schema(self) -> None:
        """Initialize the database connection and create tables idempotently."""
        await self._ensure_conn()

    async def _ensure_conn(self) -> aiosqlite.Connection:
        if self._conn is None:
            self._db_path.parent.mkdir(parents=True, exist_ok=True)
            self._conn = await aiosqlite.connect(str(self._db_path), isolation_level=None)
            await self._conn.execute("PRAGMA journal_mode=WAL")
            await self._conn.execute("PRAGMA foreign_keys=ON")
            await self._conn.executescript(SQLITE_ALL)
        return self._conn

    @asynccontextmanager
    async def _transaction(self) -> AsyncIterator[aiosqlite.Connection]:
        """Begin an IMMEDIATE transaction, commit on success, rollback on error.

        Yields:
            The aiosqlite connection inside the active transaction.
        """
        conn = await self._ensure_conn()
        await conn.execute("BEGIN IMMEDIATE")
        try:
            yield conn
            await conn.commit()
        except BaseException:
            await conn.rollback()
            raise

    # ── EventStore ────────────────────────────────────────────────────────

    async def append(self, event: Event) -> None:
        """Append an event to the log."""
        event_id = uuid.uuid4().hex
        event_type = type(event).__name__
        payload = json.dumps(event.model_dump(mode="json"))
        async with self._transaction() as conn:
            await conn.execute(
                "INSERT INTO events "
                "(event_id, timestamp, workflow_id, event_type, payload) "
                "VALUES (?, ?, ?, ?, ?)",
                (event_id, event.timestamp, event.workflow_id, event_type, payload),
            )

    async def query_events(
        self,
        *,
        dispatch_id: str | None = None,
        event_type: str | None = None,
        since: str | None = None,
        until: str | None = None,
        limit: int = 50,
        offset: int = 0,
    ) -> EventQueryResult:
        """Query events with optional filters and pagination."""
        conn = await self._ensure_conn()
        clauses: list[str] = []
        params: list[str | int] = []

        if dispatch_id is not None:
            clauses.append("workflow_id = ?")
            params.append(dispatch_id)
        if event_type is not None:
            clauses.append("event_type = ?")
            params.append(event_type)
        if since is not None:
            clauses.append("timestamp >= ?")
            params.append(since)
        if until is not None:
            clauses.append("timestamp <= ?")
            params.append(until)

        where = (" WHERE " + " AND ".join(clauses)) if clauses else ""

        count_sql = f"SELECT COUNT(*) FROM events{where}"
        count_cursor = await conn.execute(count_sql, params)
        row = await count_cursor.fetchone()
        total = row[0] if row else 0

        select_sql = (
            "SELECT id, event_id, timestamp, workflow_id, "
            f"event_type, payload FROM events{where} "
            "ORDER BY id LIMIT ? OFFSET ?"
        )
        params.extend([limit, offset])
        cursor = await conn.execute(select_sql, params)
        rows = await cursor.fetchall()

        events: list[EventRow] = []
        skipped = 0
        for r in rows:
            try:
                payload_data = json.loads(r[5])
            except json.JSONDecodeError, TypeError:
                skipped += 1
                continue
            events.append(
                EventRow(
                    id=r[0],
                    timestamp=r[2],
                    workflow_id=r[3],
                    event_type=r[4],
                    payload=payload_data,
                )
            )

        return EventQueryResult(events=events, total=total, skipped=skipped)

    # ── JobQueue ──────────────────────────────────────────────────────────

    async def enqueue_step(
        self,
        *,
        step_id: str,
        dispatch_id: str,
        step_type: str,
        step_sequence: int,
        lane: str | None,
        payload_json: str,
    ) -> None:
        """Insert a step and append StepEnqueued event, atomically."""
        now = _now()
        async with self._transaction() as conn:
            await conn.execute(
                "INSERT INTO step_projection "
                "(step_id, dispatch_id, step_type, step_sequence, lane, "
                "status, payload_json, retry_count, created_at, updated_at) "
                "VALUES (?, ?, ?, ?, ?, 'pending', ?, 0, ?, ?)",
                (step_id, dispatch_id, step_type, step_sequence, lane, payload_json, now, now),
            )
            event = StepEnqueued(
                timestamp=now,
                workflow_id=dispatch_id,
                step_id=step_id,
                step_type=StepType(step_type),
                step_sequence=step_sequence,
                lane=Lane(lane) if lane else None,
            )
            event_payload = json.dumps(event.model_dump(mode="json"))
            await conn.execute(
                "INSERT INTO events "
                "(event_id, timestamp, workflow_id, event_type, payload) "
                "VALUES (?, ?, ?, ?, ?)",
                (uuid.uuid4().hex, now, dispatch_id, "StepEnqueued", event_payload),
            )
            await conn.execute(
                "UPDATE dispatch_projection "
                "SET status = 'running', updated_at = ? "
                "WHERE dispatch_id = ? AND status = 'pending'",
                (now, dispatch_id),
            )

    async def dequeue(
        self,
        *,
        lane: Lane | None = None,
        worker_id: str,
        max_concurrent: int,
    ) -> QueuedStep | None:
        """Atomically claim a pending step if capacity allows.

        Uses manual transaction control because early-return paths
        require explicit rollback before returning None.
        """
        conn = await self._ensure_conn()
        await conn.execute("BEGIN IMMEDIATE")
        try:
            if lane is not None:
                cursor = await conn.execute(
                    "SELECT COUNT(*) FROM step_projection WHERE lane = ? AND status = 'running'",
                    (str(lane),),
                )
            else:
                cursor = await conn.execute(
                    "SELECT COUNT(*) FROM step_projection "
                    "WHERE lane IS NULL AND status = 'running'",
                )
            row = await cursor.fetchone()
            running = row[0] if row else 0

            if running >= max_concurrent:
                await conn.rollback()
                return None

            cols = "step_id, dispatch_id, step_type, step_sequence, lane, payload_json"
            if lane is not None:
                cursor = await conn.execute(
                    f"SELECT {cols} FROM step_projection "
                    "WHERE lane = ? AND status = 'pending' "
                    "ORDER BY step_sequence, created_at LIMIT 1",
                    (str(lane),),
                )
            else:
                cursor = await conn.execute(
                    f"SELECT {cols} FROM step_projection "
                    "WHERE lane IS NULL AND status = 'pending' "
                    "ORDER BY step_sequence, created_at LIMIT 1",
                )
            row = await cursor.fetchone()
            if row is None:
                await conn.rollback()
                return None

            sid, did, stype, seq, slane, pjson = row
            now = _now()

            await conn.execute(
                "UPDATE step_projection "
                "SET status = 'running', worker_id = ?, updated_at = ? "
                "WHERE step_id = ?",
                (worker_id, now, sid),
            )
            await conn.commit()

            return QueuedStep(
                step_id=sid,
                dispatch_id=did,
                step_type=StepType(stype),
                step_sequence=seq,
                lane=Lane(slane) if slane else None,
                payload_json=pjson,
            )
        except BaseException:
            await conn.rollback()
            raise

    async def ack(self, step_id: str, *, result_json: str) -> None:
        """Mark step completed with result."""
        now = _now()
        async with self._transaction() as conn:
            await conn.execute(
                "UPDATE step_projection "
                "SET status = 'completed', result_json = ?, error = NULL, updated_at = ? "
                "WHERE step_id = ?",
                (result_json, now, step_id),
            )

    async def ack_and_enqueue(
        self,
        step_id: str,
        *,
        result_json: str,
        next_step_id: str,
        next_dispatch_id: str,
        next_step_type: str,
        next_step_sequence: int,
        next_lane: str | None,
        next_payload_json: str,
        completion_events: list[Event] | None = None,
    ) -> None:
        """Atomically ack a step and enqueue the next step in one transaction."""
        now = _now()
        async with self._transaction() as conn:
            # 1. Ack: mark current step completed
            await conn.execute(
                "UPDATE step_projection "
                "SET status = 'completed', result_json = ?, error = NULL, updated_at = ? "
                "WHERE step_id = ?",
                (result_json, now, step_id),
            )
            # 2. Insert completion events (if any)
            if completion_events:
                for evt in completion_events:
                    evt_id = uuid.uuid4().hex
                    evt_type = type(evt).__name__
                    evt_payload = json.dumps(evt.model_dump(mode="json"))
                    await conn.execute(
                        "INSERT INTO events "
                        "(event_id, timestamp, workflow_id, event_type, payload) "
                        "VALUES (?, ?, ?, ?, ?)",
                        (evt_id, evt.timestamp, evt.workflow_id, evt_type, evt_payload),
                    )
            # 3. Enqueue: insert next step
            await conn.execute(
                "INSERT INTO step_projection "
                "(step_id, dispatch_id, step_type, step_sequence, lane, "
                "status, payload_json, retry_count, created_at, updated_at) "
                "VALUES (?, ?, ?, ?, ?, 'pending', ?, 0, ?, ?)",
                (
                    next_step_id,
                    next_dispatch_id,
                    next_step_type,
                    next_step_sequence,
                    next_lane,
                    next_payload_json,
                    now,
                    now,
                ),
            )
            # 4. Append StepEnqueued event
            event = StepEnqueued(
                timestamp=now,
                workflow_id=next_dispatch_id,
                step_id=next_step_id,
                step_type=StepType(next_step_type),
                step_sequence=next_step_sequence,
                lane=Lane(next_lane) if next_lane else None,
            )
            event_payload = json.dumps(event.model_dump(mode="json"))
            await conn.execute(
                "INSERT INTO events "
                "(event_id, timestamp, workflow_id, event_type, payload) "
                "VALUES (?, ?, ?, ?, ?)",
                (uuid.uuid4().hex, now, next_dispatch_id, "StepEnqueued", event_payload),
            )
            # 5. Update dispatch status to running (if still pending)
            await conn.execute(
                "UPDATE dispatch_projection "
                "SET status = 'running', updated_at = ? "
                "WHERE dispatch_id = ? AND status = 'pending'",
                (now, next_dispatch_id),
            )

    async def cancel_pending_steps(self, dispatch_id: str) -> int:
        """Cancel pending forward-progress steps for a dispatch.

        Teardown steps are excluded so resource cleanup still runs.
        """
        conn = await self._ensure_conn()
        now = _now()
        cursor = await conn.execute(
            "UPDATE step_projection "
            "SET status = 'cancelled', updated_at = ? "
            "WHERE dispatch_id = ? AND status = 'pending' "
            "AND step_type != 'teardown'",
            (now, dispatch_id),
        )
        await conn.commit()
        return cursor.rowcount

    async def nack(
        self,
        step_id: str,
        *,
        error: str,
        error_class: str | None = None,  # noqa: ARG002 — reserved for future classification filtering
        retry: bool = False,
    ) -> None:
        """Mark step failed or re-enqueue for retry."""
        now = _now()
        async with self._transaction() as conn:
            if retry:
                await conn.execute(
                    "UPDATE step_projection "
                    "SET status = 'pending', error = ?, "
                    "retry_count = retry_count + 1, "
                    "worker_id = NULL, updated_at = ? "
                    "WHERE step_id = ?",
                    (error, now, step_id),
                )
            else:
                await conn.execute(
                    "UPDATE step_projection "
                    "SET status = 'failed', error = ?, updated_at = ? "
                    "WHERE step_id = ?",
                    (error, now, step_id),
                )

    # ── StateStore ────────────────────────────────────────────────────────

    async def get_dispatch(
        self,
        dispatch_id: str,
    ) -> DispatchView | None:
        """Look up a dispatch by ID."""
        conn = await self._ensure_conn()
        cursor = await conn.execute(
            "SELECT dispatch_id, mode, status, outcome, lane, "
            "preserve_on_failure, dispatch_json, created_at, updated_at "
            "FROM dispatch_projection WHERE dispatch_id = ?",
            (dispatch_id,),
        )
        row = await cursor.fetchone()
        if row is None:
            return None
        return self._row_to_dispatch_view(row)

    async def query_dispatches(
        self,
        filters: DispatchListFilter,
    ) -> list[DispatchView]:
        """Query dispatches with filters."""
        conn = await self._ensure_conn()
        clauses: list[str] = []
        params: list[str | int] = []

        if filters.status is not None:
            clauses.append("status = ?")
            params.append(str(filters.status))
        if filters.lane is not None:
            clauses.append("lane = ?")
            params.append(str(filters.lane))
        if filters.project is not None:
            clauses.append("json_extract(dispatch_json, '$.project') = ?")
            params.append(filters.project)
        if filters.since is not None:
            clauses.append("created_at >= ?")
            params.append(filters.since)
        if filters.until is not None:
            clauses.append("created_at <= ?")
            params.append(filters.until)

        where = (" WHERE " + " AND ".join(clauses)) if clauses else ""
        query = (
            "SELECT dispatch_id, mode, status, outcome, lane, "
            "preserve_on_failure, dispatch_json, created_at, updated_at "
            f"FROM dispatch_projection{where} "
            "ORDER BY created_at DESC LIMIT ? OFFSET ?"
        )
        params.extend([filters.limit, filters.offset])
        cursor = await conn.execute(query, params)
        rows = await cursor.fetchall()
        return [self._row_to_dispatch_view(r) for r in rows]

    async def get_step(
        self,
        step_id: str,
    ) -> StepView | None:
        """Look up a step by ID."""
        conn = await self._ensure_conn()
        cursor = await conn.execute(
            "SELECT step_id, dispatch_id, step_type, step_sequence, "
            "lane, status, worker_id, result_json, error, retry_count, "
            "created_at, updated_at "
            "FROM step_projection WHERE step_id = ?",
            (step_id,),
        )
        row = await cursor.fetchone()
        if row is None:
            return None
        return self._row_to_step_view(row)

    async def get_steps_for_dispatch(
        self,
        dispatch_id: str,
    ) -> list[StepView]:
        """Get all steps for a dispatch, ordered by step_sequence."""
        conn = await self._ensure_conn()
        cursor = await conn.execute(
            "SELECT step_id, dispatch_id, step_type, step_sequence, "
            "lane, status, worker_id, result_json, error, retry_count, "
            "created_at, updated_at "
            "FROM step_projection WHERE dispatch_id = ? "
            "ORDER BY step_sequence",
            (dispatch_id,),
        )
        rows = await cursor.fetchall()
        return [self._row_to_step_view(r) for r in rows]

    async def count_running_steps(
        self,
        *,
        lane: Lane | None = None,
    ) -> int:
        """Count running steps for a lane."""
        conn = await self._ensure_conn()
        if lane is not None:
            cursor = await conn.execute(
                "SELECT COUNT(*) FROM step_projection WHERE lane = ? AND status = 'running'",
                (str(lane),),
            )
        else:
            cursor = await conn.execute(
                "SELECT COUNT(*) FROM step_projection WHERE status = 'running'",
            )
        row = await cursor.fetchone()
        return row[0] if row else 0

    # ── Dispatch projection helpers ───────────────────────────────────────

    async def create_dispatch_projection(
        self,
        *,
        dispatch_id: str,
        mode: DispatchMode,
        lane: Lane,
        preserve_on_failure: bool,
        dispatch_json: str,
    ) -> None:
        """Insert a new dispatch projection row."""
        now = _now()
        async with self._transaction() as conn:
            await conn.execute(
                "INSERT INTO dispatch_projection "
                "(dispatch_id, mode, status, lane, "
                "preserve_on_failure, dispatch_json, "
                "created_at, updated_at) "
                "VALUES (?, ?, 'pending', ?, ?, ?, ?, ?)",
                (
                    dispatch_id,
                    str(mode),
                    str(lane),
                    int(preserve_on_failure),
                    dispatch_json,
                    now,
                    now,
                ),
            )

    async def update_dispatch_status(
        self,
        dispatch_id: str,
        status: DispatchStatus,
        outcome: Outcome | None = None,
    ) -> None:
        """Update dispatch projection status."""
        now = _now()
        async with self._transaction() as conn:
            await conn.execute(
                "UPDATE dispatch_projection "
                "SET status = ?, outcome = ?, updated_at = ? "
                "WHERE dispatch_id = ?",
                (str(status), str(outcome) if outcome else None, now, dispatch_id),
            )

    # ── Lifecycle ─────────────────────────────────────────────────────────

    async def close(self) -> None:
        """Close the database connection."""
        if self._conn is not None:
            await self._conn.close()
            self._conn = None

    # ── Internal helpers ──────────────────────────────────────────────────

    @staticmethod
    def _row_to_dispatch_view(row: aiosqlite.Row) -> DispatchView:  # type is tuple at runtime
        dispatch_str = row[6] if isinstance(row[6], str) else json.dumps(row[6])
        return DispatchView(
            dispatch_id=str(row[0]),
            mode=DispatchMode(str(row[1])),
            status=DispatchStatus(str(row[2])),
            outcome=Outcome(str(row[3])) if row[3] else None,
            lane=Lane(str(row[4])),
            preserve_on_failure=bool(row[5]),
            dispatch=Dispatch.model_validate_json(dispatch_str),
            created_at=str(row[7]),
            updated_at=str(row[8]),
        )

    @staticmethod
    def _row_to_step_view(row: aiosqlite.Row) -> StepView:  # type is tuple at runtime
        return StepView(
            step_id=str(row[0]),
            dispatch_id=str(row[1]),
            step_type=StepType(str(row[2])),
            step_sequence=int(str(row[3])),
            lane=Lane(str(row[4])) if row[4] else None,
            status=StepStatus(str(row[5])),
            worker_id=str(row[6]) if row[6] else None,
            result_json=str(row[7]) if row[7] else None,
            error=str(row[8]) if row[8] else None,
            retry_count=int(str(row[9])),
            created_at=str(row[10]),
            updated_at=str(row[11]),
        )
