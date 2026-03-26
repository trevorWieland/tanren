"""SQLite implementation of EventStore, JobQueue, and StateStore protocols.

A single ``SqliteStore`` instance owns one ``aiosqlite`` connection and
implements all three protocols.  It uses WAL mode for concurrent reads and
``BEGIN IMMEDIATE`` for writes to prevent SQLITE_BUSY under contention.
"""

from __future__ import annotations

import asyncio
import json
import uuid
from collections.abc import AsyncIterator
from contextlib import asynccontextmanager
from datetime import UTC, datetime, timedelta
from pathlib import Path
from typing import TYPE_CHECKING

import aiosqlite

from tanren_core.adapters.events import Event
from tanren_core.schemas import Dispatch, Outcome
from tanren_core.store.auth_events import ResourceLimits
from tanren_core.store.auth_views import ApiKeyView, UserView
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
        self._lock = asyncio.Lock()

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

        Serialized via lock to prevent concurrent transactions on the
        single aiosqlite connection.

        Yields:
            The aiosqlite connection inside the active transaction.
        """
        async with self._lock:
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
                "(event_id, timestamp, entity_id, entity_type, event_type, payload) "
                "VALUES (?, ?, ?, ?, ?, ?)",
                (
                    event_id,
                    event.timestamp,
                    event.entity_id,
                    str(event.entity_type),
                    event_type,
                    payload,
                ),
            )

    async def query_events(
        self,
        *,
        entity_id: str | None = None,
        entity_type: str | None = None,
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

        if entity_id is not None:
            clauses.append("entity_id = ?")
            params.append(entity_id)
        if entity_type is not None:
            clauses.append("entity_type = ?")
            params.append(entity_type)
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
            "SELECT id, event_id, timestamp, entity_id, entity_type, "
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
                payload_data = json.loads(r[6])
            except json.JSONDecodeError, TypeError:
                skipped += 1
                continue
            events.append(
                EventRow(
                    id=r[0],
                    timestamp=r[2],
                    entity_id=r[3],
                    entity_type=r[4],
                    event_type=r[5],
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
                entity_id=dispatch_id,
                step_id=step_id,
                step_type=StepType(step_type),
                step_sequence=step_sequence,
                lane=Lane(lane) if lane else None,
            )
            event_payload = json.dumps(event.model_dump(mode="json"))
            await conn.execute(
                "INSERT INTO events "
                "(event_id, timestamp, entity_id, entity_type, event_type, payload) "
                "VALUES (?, ?, ?, ?, ?, ?)",
                (
                    uuid.uuid4().hex,
                    now,
                    dispatch_id,
                    str(event.entity_type),
                    "StepEnqueued",
                    event_payload,
                ),
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
        async with self._lock:
            return await self._dequeue_locked(
                lane=lane, worker_id=worker_id, max_concurrent=max_concurrent
            )

    async def _dequeue_locked(
        self,
        *,
        lane: Lane | None = None,
        worker_id: str,
        max_concurrent: int,
    ) -> QueuedStep | None:
        """Dequeue implementation (caller must hold self._lock)."""
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

            # Exclude forward-progress steps from cancelled dispatches but
            # still allow teardown through (cleanup must always run)
            cols = "s.step_id, s.dispatch_id, s.step_type, s.step_sequence, s.lane, s.payload_json"
            cancelled_filter = (
                "JOIN dispatch_projection d ON s.dispatch_id = d.dispatch_id "
                "WHERE s.status = 'pending' "
                "AND (d.status != 'cancelled' OR s.step_type = 'teardown')"
            )
            if lane is not None:
                cursor = await conn.execute(
                    f"SELECT {cols} FROM step_projection s "
                    f"{cancelled_filter} AND s.lane = ? "
                    "ORDER BY s.step_sequence, s.created_at LIMIT 1",
                    (str(lane),),
                )
            else:
                # Infra lane: FIFO by enqueue time so teardowns aren't
                # starved behind provisions under sustained load
                cursor = await conn.execute(
                    f"SELECT {cols} FROM step_projection s "
                    f"{cancelled_filter} AND s.lane IS NULL "
                    "ORDER BY s.created_at, s.step_sequence LIMIT 1",
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
                        "(event_id, timestamp, entity_id, entity_type, event_type, payload) "
                        "VALUES (?, ?, ?, ?, ?, ?)",
                        (
                            evt_id,
                            evt.timestamp,
                            evt.entity_id,
                            str(evt.entity_type),
                            evt_type,
                            evt_payload,
                        ),
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
                entity_id=next_dispatch_id,
                step_id=next_step_id,
                step_type=StepType(next_step_type),
                step_sequence=next_step_sequence,
                lane=Lane(next_lane) if next_lane else None,
            )
            event_payload = json.dumps(event.model_dump(mode="json"))
            await conn.execute(
                "INSERT INTO events "
                "(event_id, timestamp, entity_id, entity_type, event_type, payload) "
                "VALUES (?, ?, ?, ?, ?, ?)",
                (
                    uuid.uuid4().hex,
                    now,
                    next_dispatch_id,
                    str(event.entity_type),
                    "StepEnqueued",
                    event_payload,
                ),
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
        now = _now()
        async with self._transaction() as conn:
            cursor = await conn.execute(
                "UPDATE step_projection "
                "SET status = 'cancelled', updated_at = ? "
                "WHERE dispatch_id = ? AND status = 'pending' "
                "AND step_type != 'teardown'",
                (now, dispatch_id),
            )
            return cursor.rowcount

    async def recover_stale_steps(self, *, timeout_secs: int = 300) -> int:
        """Reset running steps older than timeout_secs back to pending."""
        now = _now()
        cutoff = (
            (datetime.now(UTC) - timedelta(seconds=timeout_secs)).isoformat().replace("+00:00", "Z")
        )
        async with self._transaction() as conn:
            cursor = await conn.execute(
                "UPDATE step_projection "
                "SET status = 'pending', worker_id = NULL, updated_at = ? "
                "WHERE status = 'running' AND updated_at < ?",
                (now, cutoff),
            )
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
            "preserve_on_failure, dispatch_json, user_id, created_at, updated_at "
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
        if filters.user_id is not None:
            clauses.append("user_id = ?")
            params.append(filters.user_id)
        if filters.since is not None:
            clauses.append("created_at >= ?")
            params.append(filters.since)
        if filters.until is not None:
            clauses.append("created_at <= ?")
            params.append(filters.until)

        where = (" WHERE " + " AND ".join(clauses)) if clauses else ""
        query = (
            "SELECT dispatch_id, mode, status, outcome, lane, "
            "preserve_on_failure, dispatch_json, user_id, created_at, updated_at "
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
        user_id: str = "",
    ) -> None:
        """Insert a new dispatch projection row."""
        now = _now()
        async with self._transaction() as conn:
            await conn.execute(
                "INSERT INTO dispatch_projection "
                "(dispatch_id, mode, status, lane, "
                "preserve_on_failure, dispatch_json, user_id, "
                "created_at, updated_at) "
                "VALUES (?, ?, 'pending', ?, ?, ?, ?, ?, ?)",
                (
                    dispatch_id,
                    str(mode),
                    str(lane),
                    int(preserve_on_failure),
                    dispatch_json,
                    user_id,
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
        """Update dispatch projection status.

        Terminal states (completed, failed, cancelled) are protected:
        once a dispatch reaches any terminal state, further status
        updates are silently ignored to prevent race conditions
        (e.g. cancel overwriting completed, or teardown overwriting cancelled).
        """
        now = _now()
        async with self._transaction() as conn:
            await conn.execute(
                "UPDATE dispatch_projection "
                "SET status = ?, outcome = ?, updated_at = ? "
                "WHERE dispatch_id = ? "
                "AND status NOT IN ('completed', 'failed', 'cancelled')",
                (str(status), str(outcome) if outcome else None, now, dispatch_id),
            )

    # ── AuthStore: Users ──────────────────────────────────────────────────

    async def create_user(
        self,
        *,
        user_id: str,
        name: str,
        email: str | None,
        role: str,
    ) -> None:
        """Insert a new user projection row."""
        now = _now()
        async with self._transaction() as conn:
            await conn.execute(
                "INSERT INTO user_projection "
                "(user_id, name, email, role, is_active, created_at, updated_at) "
                "VALUES (?, ?, ?, ?, 1, ?, ?)",
                (user_id, name, email, role, now, now),
            )

    async def get_user(self, user_id: str) -> UserView | None:
        """Look up a user by ID."""
        conn = await self._ensure_conn()
        row = list(
            await conn.execute_fetchall(
                "SELECT user_id, name, email, role, is_active, created_at, updated_at "
                "FROM user_projection WHERE user_id = ?",
                (user_id,),
            )
        )
        if not row:
            return None
        r = row[0]
        return UserView(
            user_id=str(r[0]),
            name=str(r[1]),
            email=str(r[2]) if r[2] else None,
            role=str(r[3]),
            is_active=bool(r[4]),
            created_at=str(r[5]),
            updated_at=str(r[6]),
        )

    async def list_users(self, *, limit: int = 50, offset: int = 0) -> list[UserView]:
        """List users with pagination."""
        conn = await self._ensure_conn()
        rows = await conn.execute_fetchall(
            "SELECT user_id, name, email, role, is_active, created_at, updated_at "
            "FROM user_projection ORDER BY created_at DESC LIMIT ? OFFSET ?",
            (limit, offset),
        )
        return [
            UserView(
                user_id=str(r[0]),
                name=str(r[1]),
                email=str(r[2]) if r[2] else None,
                role=str(r[3]),
                is_active=bool(r[4]),
                created_at=str(r[5]),
                updated_at=str(r[6]),
            )
            for r in rows
        ]

    async def update_user(
        self,
        user_id: str,
        *,
        name: str | None = None,
        email: str | None = None,
        role: str | None = None,
    ) -> None:
        """Update mutable user fields."""
        sets: list[str] = []
        params: list[str | None] = []
        if name is not None:
            sets.append("name = ?")
            params.append(name)
        if email is not None:
            sets.append("email = ?")
            params.append(email)
        if role is not None:
            sets.append("role = ?")
            params.append(role)
        if not sets:
            return
        sets.append("updated_at = ?")
        params.extend((_now(), user_id))
        async with self._transaction() as conn:
            await conn.execute(
                f"UPDATE user_projection SET {', '.join(sets)} WHERE user_id = ?",
                tuple(params),
            )

    async def deactivate_user(self, user_id: str) -> None:
        """Set is_active = 0 on a user."""
        async with self._transaction() as conn:
            await conn.execute(
                "UPDATE user_projection SET is_active = 0, updated_at = ? WHERE user_id = ?",
                (_now(), user_id),
            )

    # ── AuthStore: API keys ──────────────────────────────────────────────

    async def create_api_key(
        self,
        *,
        key_id: str,
        user_id: str,
        name: str,
        key_prefix: str,
        key_hash: str,
        scopes_json: str,
        resource_limits_json: str | None,
        expires_at: str | None,
    ) -> None:
        """Insert a new API key projection row."""
        now = _now()
        async with self._transaction() as conn:
            await conn.execute(
                "INSERT INTO api_key_projection "
                "(key_id, user_id, name, key_prefix, key_hash, scopes, "
                "resource_limits, created_at, expires_at, revoked_at, grace_replaced_by) "
                "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL)",
                (
                    key_id,
                    user_id,
                    name,
                    key_prefix,
                    key_hash,
                    scopes_json,
                    resource_limits_json,
                    now,
                    expires_at,
                ),
            )

    async def get_api_key_by_hash(self, key_hash: str) -> ApiKeyView | None:
        """Look up an API key by its SHA-256 hash."""
        conn = await self._ensure_conn()
        rows = list(
            await conn.execute_fetchall(
                "SELECT key_id, user_id, name, key_prefix, key_hash, scopes, "
                "resource_limits, created_at, expires_at, revoked_at, grace_replaced_by "
                "FROM api_key_projection WHERE key_hash = ?",
                (key_hash,),
            )
        )
        if not rows:
            return None
        return self._row_to_api_key_view(rows[0])

    async def get_api_key(self, key_id: str) -> ApiKeyView | None:
        """Look up an API key by ID."""
        conn = await self._ensure_conn()
        rows = list(
            await conn.execute_fetchall(
                "SELECT key_id, user_id, name, key_prefix, key_hash, scopes, "
                "resource_limits, created_at, expires_at, revoked_at, grace_replaced_by "
                "FROM api_key_projection WHERE key_id = ?",
                (key_id,),
            )
        )
        if not rows:
            return None
        return self._row_to_api_key_view(rows[0])

    async def list_api_keys(
        self,
        *,
        user_id: str | None = None,
        include_revoked: bool = False,
        limit: int = 50,
        offset: int = 0,
    ) -> list[ApiKeyView]:
        """List API keys, optionally filtered by user."""
        clauses: list[str] = []
        params: list[str | int] = []
        if user_id is not None:
            clauses.append("user_id = ?")
            params.append(user_id)
        if not include_revoked:
            # Include keys in grace period (revoked_at set to a future timestamp)
            clauses.append("(revoked_at IS NULL OR revoked_at > ?)")
            params.append(_now())
        where = f" WHERE {' AND '.join(clauses)}" if clauses else ""
        params.extend([limit, offset])
        conn = await self._ensure_conn()
        rows = await conn.execute_fetchall(
            "SELECT key_id, user_id, name, key_prefix, key_hash, scopes, "
            "resource_limits, created_at, expires_at, revoked_at, grace_replaced_by "
            f"FROM api_key_projection{where} ORDER BY created_at DESC LIMIT ? OFFSET ?",
            tuple(params),
        )
        return [self._row_to_api_key_view(r) for r in rows]

    async def revoke_api_key(self, key_id: str) -> None:
        """Set revoked_at to now on an API key."""
        async with self._transaction() as conn:
            await conn.execute(
                "UPDATE api_key_projection SET revoked_at = ? WHERE key_id = ?",
                (_now(), key_id),
            )

    async def set_grace_replacement(
        self, key_id: str, *, replaced_by: str, revoked_at: str
    ) -> None:
        """Mark old key as replaced during rotation."""
        async with self._transaction() as conn:
            await conn.execute(
                "UPDATE api_key_projection "
                "SET grace_replaced_by = ?, revoked_at = ? WHERE key_id = ?",
                (replaced_by, revoked_at, key_id),
            )

    # ── AuthStore: Resource limit queries ────────────────────────────────

    async def count_dispatches_since(self, user_id: str, since: str) -> int:
        """Count dispatches created by user since timestamp."""
        conn = await self._ensure_conn()
        rows = list(
            await conn.execute_fetchall(
                "SELECT COUNT(*) FROM dispatch_projection WHERE user_id = ? AND created_at >= ?",
                (user_id, since),
            )
        )
        return int(str(rows[0][0]))

    async def count_active_vms(self, user_id: str) -> int:
        """Count VMs currently active for user.

        A VM is active when its dispatch has a completed provision step
        but no completed teardown step.
        """
        conn = await self._ensure_conn()
        rows = list(
            await conn.execute_fetchall(
                "SELECT COUNT(DISTINCT sp1.dispatch_id) FROM step_projection sp1 "
                "JOIN dispatch_projection dp ON sp1.dispatch_id = dp.dispatch_id "
                "WHERE dp.user_id = ? "
                "AND sp1.step_type = 'provision' AND sp1.status = 'completed' "
                "AND NOT EXISTS ("
                "  SELECT 1 FROM step_projection sp2 "
                "  WHERE sp2.dispatch_id = sp1.dispatch_id "
                "  AND sp2.step_type = 'teardown' AND sp2.status = 'completed'"
                ")",
                (user_id,),
            )
        )
        return int(str(rows[0][0]))

    async def sum_cost_since(self, user_id: str, since: str) -> float:
        """Sum USD cost from TokenUsageRecorded events for user since timestamp."""
        conn = await self._ensure_conn()
        rows = list(
            await conn.execute_fetchall(
                "SELECT COALESCE(SUM(CAST(json_extract(payload, '$.total_cost') AS REAL)), 0.0) "
                "FROM events "
                "WHERE entity_type = 'dispatch' AND event_type = 'TokenUsageRecorded' "
                "AND entity_id IN (SELECT dispatch_id FROM dispatch_projection WHERE user_id = ?) "
                "AND timestamp >= ?",
                (user_id, since),
            )
        )
        return float(str(rows[0][0]))

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
            user_id=str(row[7]) if row[7] else "",
            created_at=str(row[8]),
            updated_at=str(row[9]),
        )

    @staticmethod
    def _row_to_api_key_view(row: aiosqlite.Row) -> ApiKeyView:  # type is tuple at runtime
        scopes_raw = row[5]
        if isinstance(scopes_raw, str):
            scopes = json.loads(scopes_raw)
        else:
            scopes = list(scopes_raw) if scopes_raw else []
        rl_raw = row[6]
        resource_limits = None
        if rl_raw:
            rl_str = rl_raw if isinstance(rl_raw, str) else json.dumps(rl_raw)
            resource_limits = ResourceLimits.model_validate_json(rl_str)
        return ApiKeyView(
            key_id=str(row[0]),
            user_id=str(row[1]),
            name=str(row[2]),
            key_prefix=str(row[3]),
            key_hash=str(row[4]),
            scopes=scopes,
            resource_limits=resource_limits,
            created_at=str(row[7]),
            expires_at=str(row[8]) if row[8] else None,
            revoked_at=str(row[9]) if row[9] else None,
            grace_replaced_by=str(row[10]) if row[10] else None,
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
