"""Postgres implementation of EventStore, JobQueue, and StateStore protocols.

A single ``PostgresStore`` instance wraps an externally-owned ``asyncpg.Pool``
and implements all three protocols.  It uses ``SELECT ... FOR UPDATE SKIP
LOCKED`` for safe concurrent dequeue operations.
"""

from __future__ import annotations

import json
import uuid
from datetime import UTC, datetime, timedelta

import asyncpg

from tanren_core.adapters.events import Event
from tanren_core.schemas import Dispatch, Outcome
from tanren_core.store.auth_events import ResourceLimits
from tanren_core.store.auth_views import ApiKeyView, UserView
from tanren_core.store.enums import (
    DispatchMode,
    DispatchStatus,
    EntityType,
    Lane,
    StepStatus,
    StepType,
)
from tanren_core.store.events import StepEnqueued
from tanren_core.store.schema import POSTGRES_ALL
from tanren_core.store.views import (
    DispatchListFilter,
    DispatchView,
    EventQueryResult,
    EventRow,
    QueuedStep,
    StepView,
)


def _now() -> str:
    return datetime.now(UTC).isoformat().replace("+00:00", "Z")


class PostgresStore:
    """Unified Postgres store implementing EventStore, JobQueue, and StateStore."""

    def __init__(self, pool: asyncpg.Pool, *, owns_pool: bool = False) -> None:
        """Initialise with an asyncpg connection pool.

        When *owns_pool* is True (e.g. when the factory creates the pool),
        ``close()`` will shut down the pool.  Otherwise the caller is
        responsible for pool lifecycle.
        """
        self._pool = pool
        self._owns_pool = owns_pool

    async def ensure_schema(self) -> None:
        """Create store tables idempotently."""
        async with self._pool.acquire() as conn:
            for statement in POSTGRES_ALL.strip().split(";"):
                statement = statement.strip()
                if statement:
                    await conn.execute(statement)

    # ── EventStore ────────────────────────────────────────────────────────

    async def append(self, event: Event) -> None:
        """Append an event to the log."""
        event_id = uuid.uuid4().hex
        event_type = type(event).__name__
        payload = json.dumps(event.model_dump(mode="json"))
        async with self._pool.acquire() as conn, conn.transaction():
            await conn.execute(
                "INSERT INTO events "
                "(event_id, timestamp, entity_id, entity_type, event_type, payload) "
                "VALUES ($1, $2, $3, $4, $5, $6)",
                event_id,
                event.timestamp,
                event.entity_id,
                str(event.entity_type),
                event_type,
                payload,
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
        clauses: list[str] = []
        params: list[str | int] = []
        idx = 1

        if entity_id is not None:
            clauses.append(f"entity_id = ${idx}")
            params.append(entity_id)
            idx += 1
        if entity_type is not None:
            clauses.append(f"entity_type = ${idx}")
            params.append(entity_type)
            idx += 1
        if event_type is not None:
            clauses.append(f"event_type = ${idx}")
            params.append(event_type)
            idx += 1
        if since is not None:
            clauses.append(f"timestamp >= ${idx}")
            params.append(since)
            idx += 1
        if until is not None:
            clauses.append(f"timestamp <= ${idx}")
            params.append(until)
            idx += 1

        where = (" WHERE " + " AND ".join(clauses)) if clauses else ""

        count_sql = f"SELECT COUNT(*) FROM events{where}"
        row = await self._pool.fetchrow(count_sql, *params)
        total = row["count"] if row else 0

        select_sql = (
            "SELECT id, event_id, timestamp, entity_id, entity_type, "
            f"event_type, payload FROM events{where} "
            f"ORDER BY id LIMIT ${idx} OFFSET ${idx + 1}"
        )
        params.extend([limit, offset])
        rows = await self._pool.fetch(select_sql, *params)

        events: list[EventRow] = []
        skipped = 0
        for r in rows:
            payload_data = r["payload"]
            # asyncpg returns JSONB as dict; handle str fallback
            if isinstance(payload_data, str):
                try:
                    payload_data = json.loads(payload_data)
                except json.JSONDecodeError, TypeError:
                    skipped += 1
                    continue
            events.append(
                EventRow(
                    id=r["id"],
                    timestamp=r["timestamp"],
                    entity_id=r["entity_id"],
                    entity_type=r["entity_type"],
                    event_type=r["event_type"],
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
        async with self._pool.acquire() as conn, conn.transaction():
            await conn.execute(
                "INSERT INTO step_projection "
                "(step_id, dispatch_id, step_type, step_sequence, lane, "
                "status, payload_json, retry_count, created_at, updated_at) "
                "VALUES ($1, $2, $3, $4, $5, 'pending', $6, 0, $7, $8)",
                step_id,
                dispatch_id,
                step_type,
                step_sequence,
                lane,
                payload_json,
                now,
                now,
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
                "VALUES ($1, $2, $3, $4, $5, $6)",
                uuid.uuid4().hex,
                now,
                dispatch_id,
                str(EntityType.DISPATCH),
                "StepEnqueued",
                event_payload,
            )
            await conn.execute(
                "UPDATE dispatch_projection "
                "SET status = 'running', updated_at = $1 "
                "WHERE dispatch_id = $2 AND status = 'pending'",
                now,
                dispatch_id,
            )

    async def dequeue(
        self,
        *,
        lane: Lane | None = None,
        worker_id: str,
        max_concurrent: int,
    ) -> QueuedStep | None:
        """Atomically claim a pending step if capacity allows.

        Uses ``pg_advisory_xact_lock`` per lane to serialize concurrent
        dequeue attempts, preventing TOCTOU races on the running-count
        check.
        """
        async with self._pool.acquire() as conn, conn.transaction():
            # Serialize dequeue per lane so running-count + claim is atomic
            lane_key = f"dequeue-{lane}" if lane is not None else "dequeue-infra"
            await conn.execute("SELECT pg_advisory_xact_lock(hashtext($1))", lane_key)

            # Check running count
            if lane is not None:
                row = await conn.fetchrow(
                    "SELECT COUNT(*) FROM step_projection WHERE lane = $1 AND status = 'running'",
                    str(lane),
                )
            else:
                row = await conn.fetchrow(
                    "SELECT COUNT(*) FROM step_projection "
                    "WHERE lane IS NULL AND status = 'running'",
                )
            running = row["count"] if row else 0

            if running >= max_concurrent:
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
                row = await conn.fetchrow(
                    f"SELECT {cols} FROM step_projection s "
                    f"{cancelled_filter} AND s.lane = $1 "
                    "ORDER BY s.step_sequence, s.created_at "
                    "LIMIT 1 FOR UPDATE SKIP LOCKED",
                    str(lane),
                )
            else:
                # Infra lane: FIFO by enqueue time so teardowns aren't
                # starved behind provisions under sustained load
                row = await conn.fetchrow(
                    f"SELECT {cols} FROM step_projection s "
                    f"{cancelled_filter} AND s.lane IS NULL "
                    "ORDER BY s.created_at, s.step_sequence "
                    "LIMIT 1 FOR UPDATE SKIP LOCKED",
                )
            if row is None:
                return None

            sid = row["step_id"]
            did = row["dispatch_id"]
            stype = row["step_type"]
            seq = row["step_sequence"]
            slane = row["lane"]
            pjson = row["payload_json"]

            # asyncpg returns JSONB as dict; we need a string
            if isinstance(pjson, dict):
                pjson = json.dumps(pjson)

            now = _now()
            await conn.execute(
                "UPDATE step_projection "
                "SET status = 'running', worker_id = $1, updated_at = $2 "
                "WHERE step_id = $3",
                worker_id,
                now,
                sid,
            )

            return QueuedStep(
                step_id=sid,
                dispatch_id=did,
                step_type=StepType(stype),
                step_sequence=seq,
                lane=Lane(slane) if slane else None,
                payload_json=pjson,
            )

    async def ack(self, step_id: str, *, result_json: str) -> None:
        """Mark step completed with result."""
        now = _now()
        async with self._pool.acquire() as conn, conn.transaction():
            await conn.execute(
                "UPDATE step_projection "
                "SET status = 'completed', result_json = $1, error = NULL, updated_at = $2 "
                "WHERE step_id = $3",
                result_json,
                now,
                step_id,
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
        async with self._pool.acquire() as conn, conn.transaction():
            # 1. Ack: mark current step completed
            await conn.execute(
                "UPDATE step_projection "
                "SET status = 'completed', result_json = $1, error = NULL, updated_at = $2 "
                "WHERE step_id = $3",
                result_json,
                now,
                step_id,
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
                        "VALUES ($1, $2, $3, $4, $5, $6)",
                        evt_id,
                        evt.timestamp,
                        evt.entity_id,
                        str(evt.entity_type),
                        evt_type,
                        evt_payload,
                    )
            # 3. Enqueue: insert next step
            await conn.execute(
                "INSERT INTO step_projection "
                "(step_id, dispatch_id, step_type, step_sequence, lane, "
                "status, payload_json, retry_count, created_at, updated_at) "
                "VALUES ($1, $2, $3, $4, $5, 'pending', $6, 0, $7, $8)",
                next_step_id,
                next_dispatch_id,
                next_step_type,
                next_step_sequence,
                next_lane,
                next_payload_json,
                now,
                now,
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
                "VALUES ($1, $2, $3, $4, $5, $6)",
                uuid.uuid4().hex,
                now,
                next_dispatch_id,
                str(EntityType.DISPATCH),
                "StepEnqueued",
                event_payload,
            )
            # 5. Update dispatch status to running (if still pending)
            await conn.execute(
                "UPDATE dispatch_projection "
                "SET status = 'running', updated_at = $1 "
                "WHERE dispatch_id = $2 AND status = 'pending'",
                now,
                next_dispatch_id,
            )

    async def cancel_pending_steps(self, dispatch_id: str) -> int:
        """Cancel pending forward-progress steps for a dispatch.

        Teardown steps are excluded so resource cleanup still runs.
        """
        now = _now()
        async with self._pool.acquire() as conn, conn.transaction():
            result = await conn.execute(
                "UPDATE step_projection "
                "SET status = 'cancelled', updated_at = $1 "
                "WHERE dispatch_id = $2 AND status = 'pending' "
                "AND step_type != 'teardown'",
                now,
                dispatch_id,
            )
            # asyncpg returns "UPDATE N"
            return int(result.split()[-1])

    async def recover_stale_steps(self, *, timeout_secs: int = 300) -> int:
        """Reset running steps older than timeout_secs back to pending."""
        now = _now()
        cutoff = (
            (datetime.now(UTC) - timedelta(seconds=timeout_secs)).isoformat().replace("+00:00", "Z")
        )
        async with self._pool.acquire() as conn, conn.transaction():
            result = await conn.execute(
                "UPDATE step_projection "
                "SET status = 'pending', worker_id = NULL, updated_at = $1 "
                "WHERE status = 'running' AND updated_at < $2",
                now,
                cutoff,
            )
            return int(result.split()[-1])

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
        async with self._pool.acquire() as conn, conn.transaction():
            if retry:
                await conn.execute(
                    "UPDATE step_projection "
                    "SET status = 'pending', error = $1, "
                    "retry_count = retry_count + 1, "
                    "worker_id = NULL, updated_at = $2 "
                    "WHERE step_id = $3",
                    error,
                    now,
                    step_id,
                )
            else:
                await conn.execute(
                    "UPDATE step_projection "
                    "SET status = 'failed', error = $1, updated_at = $2 "
                    "WHERE step_id = $3",
                    error,
                    now,
                    step_id,
                )

    # ── StateStore ────────────────────────────────────────────────────────

    async def get_dispatch(
        self,
        dispatch_id: str,
    ) -> DispatchView | None:
        """Look up a dispatch by ID."""
        row = await self._pool.fetchrow(
            "SELECT dispatch_id, mode, status, outcome, lane, "
            "preserve_on_failure, dispatch_json, user_id, created_at, updated_at "
            "FROM dispatch_projection WHERE dispatch_id = $1",
            dispatch_id,
        )
        if row is None:
            return None
        return self._row_to_dispatch_view(row)

    async def query_dispatches(
        self,
        filters: DispatchListFilter,
    ) -> list[DispatchView]:
        """Query dispatches with filters."""
        clauses: list[str] = []
        params: list[str | int] = []
        idx = 1

        if filters.status is not None:
            clauses.append(f"status = ${idx}")
            params.append(str(filters.status))
            idx += 1
        if filters.lane is not None:
            clauses.append(f"lane = ${idx}")
            params.append(str(filters.lane))
            idx += 1
        if filters.project is not None:
            clauses.append(f"dispatch_json->>'project' = ${idx}")
            params.append(filters.project)
            idx += 1
        if filters.user_id is not None:
            clauses.append(f"user_id = ${idx}")
            params.append(filters.user_id)
            idx += 1
        if filters.since is not None:
            clauses.append(f"created_at >= ${idx}")
            params.append(filters.since)
            idx += 1
        if filters.until is not None:
            clauses.append(f"created_at <= ${idx}")
            params.append(filters.until)
            idx += 1

        where = (" WHERE " + " AND ".join(clauses)) if clauses else ""
        query = (
            "SELECT dispatch_id, mode, status, outcome, lane, "
            "preserve_on_failure, dispatch_json, user_id, created_at, updated_at "
            f"FROM dispatch_projection{where} "
            f"ORDER BY created_at DESC LIMIT ${idx} OFFSET ${idx + 1}"
        )
        params.extend([filters.limit, filters.offset])
        rows = await self._pool.fetch(query, *params)
        return [self._row_to_dispatch_view(r) for r in rows]

    async def get_step(
        self,
        step_id: str,
    ) -> StepView | None:
        """Look up a step by ID."""
        row = await self._pool.fetchrow(
            "SELECT step_id, dispatch_id, step_type, step_sequence, "
            "lane, status, worker_id, result_json, error, retry_count, "
            "created_at, updated_at "
            "FROM step_projection WHERE step_id = $1",
            step_id,
        )
        if row is None:
            return None
        return self._row_to_step_view(row)

    async def get_steps_for_dispatch(
        self,
        dispatch_id: str,
    ) -> list[StepView]:
        """Get all steps for a dispatch, ordered by step_sequence."""
        rows = await self._pool.fetch(
            "SELECT step_id, dispatch_id, step_type, step_sequence, "
            "lane, status, worker_id, result_json, error, retry_count, "
            "created_at, updated_at "
            "FROM step_projection WHERE dispatch_id = $1 "
            "ORDER BY step_sequence",
            dispatch_id,
        )
        return [self._row_to_step_view(r) for r in rows]

    async def count_running_steps(
        self,
        *,
        lane: Lane | None = None,
    ) -> int:
        """Count running steps for a lane."""
        if lane is not None:
            row = await self._pool.fetchrow(
                "SELECT COUNT(*) FROM step_projection WHERE lane = $1 AND status = 'running'",
                str(lane),
            )
        else:
            row = await self._pool.fetchrow(
                "SELECT COUNT(*) FROM step_projection WHERE status = 'running'",
            )
        return row["count"] if row else 0

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
        async with self._pool.acquire() as conn, conn.transaction():
            await conn.execute(
                "INSERT INTO dispatch_projection "
                "(dispatch_id, mode, status, lane, "
                "preserve_on_failure, dispatch_json, user_id, "
                "created_at, updated_at) "
                "VALUES ($1, $2, 'pending', $3, $4, $5, $6, $7, $8)",
                dispatch_id,
                str(mode),
                str(lane),
                preserve_on_failure,
                dispatch_json,
                user_id,
                now,
                now,
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
        async with self._pool.acquire() as conn, conn.transaction():
            await conn.execute(
                "UPDATE dispatch_projection "
                "SET status = $1, outcome = $2, updated_at = $3 "
                "WHERE dispatch_id = $4 "
                "AND status NOT IN ('completed', 'failed', 'cancelled')",
                str(status),
                str(outcome) if outcome else None,
                now,
                dispatch_id,
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
        async with self._pool.acquire() as conn:
            await conn.execute(
                "INSERT INTO user_projection "
                "(user_id, name, email, role, is_active, created_at, updated_at) "
                "VALUES ($1, $2, $3, $4, TRUE, $5, $6)",
                user_id,
                name,
                email,
                role,
                now,
                now,
            )

    async def get_user(self, user_id: str) -> UserView | None:
        """Look up a user by ID."""
        async with self._pool.acquire() as conn:
            row = await conn.fetchrow(
                "SELECT user_id, name, email, role, is_active, created_at, updated_at "
                "FROM user_projection WHERE user_id = $1",
                user_id,
            )
        if row is None:
            return None
        return UserView(
            user_id=str(row["user_id"]),
            name=str(row["name"]),
            email=str(row["email"]) if row["email"] else None,
            role=str(row["role"]),
            is_active=bool(row["is_active"]),
            created_at=str(row["created_at"]),
            updated_at=str(row["updated_at"]),
        )

    async def list_users(self, *, limit: int = 50, offset: int = 0) -> list[UserView]:
        """List users with pagination."""
        async with self._pool.acquire() as conn:
            rows = await conn.fetch(
                "SELECT user_id, name, email, role, is_active, created_at, updated_at "
                "FROM user_projection ORDER BY created_at DESC LIMIT $1 OFFSET $2",
                limit,
                offset,
            )
        return [
            UserView(
                user_id=str(r["user_id"]),
                name=str(r["name"]),
                email=str(r["email"]) if r["email"] else None,
                role=str(r["role"]),
                is_active=bool(r["is_active"]),
                created_at=str(r["created_at"]),
                updated_at=str(r["updated_at"]),
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
        idx = 1
        if name is not None:
            sets.append(f"name = ${idx}")
            params.append(name)
            idx += 1
        if email is not None:
            sets.append(f"email = ${idx}")
            params.append(email)
            idx += 1
        if role is not None:
            sets.append(f"role = ${idx}")
            params.append(role)
            idx += 1
        if not sets:
            return
        sets.append(f"updated_at = ${idx}")
        params.append(_now())
        idx += 1
        params.append(user_id)
        async with self._pool.acquire() as conn:
            await conn.execute(
                f"UPDATE user_projection SET {', '.join(sets)} WHERE user_id = ${idx}",
                *params,
            )

    async def deactivate_user(self, user_id: str) -> None:
        """Set is_active = FALSE on a user."""
        async with self._pool.acquire() as conn:
            await conn.execute(
                "UPDATE user_projection SET is_active = FALSE, updated_at = $1 WHERE user_id = $2",
                _now(),
                user_id,
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
        # asyncpg needs dict/list for JSONB, not raw JSON strings
        scopes_val = json.loads(scopes_json)
        limits_val = json.loads(resource_limits_json) if resource_limits_json else None
        async with self._pool.acquire() as conn:
            await conn.execute(
                "INSERT INTO api_key_projection "
                "(key_id, user_id, name, key_prefix, key_hash, scopes, "
                "resource_limits, created_at, expires_at, revoked_at, grace_replaced_by) "
                "VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NULL, NULL)",
                key_id,
                user_id,
                name,
                key_prefix,
                key_hash,
                json.dumps(scopes_val),
                json.dumps(limits_val) if limits_val else None,
                now,
                expires_at,
            )

    async def get_api_key_by_hash(self, key_hash: str) -> ApiKeyView | None:
        """Look up an API key by its SHA-256 hash."""
        async with self._pool.acquire() as conn:
            row = await conn.fetchrow(
                "SELECT key_id, user_id, name, key_prefix, key_hash, scopes, "
                "resource_limits, created_at, expires_at, revoked_at, grace_replaced_by "
                "FROM api_key_projection WHERE key_hash = $1",
                key_hash,
            )
        if row is None:
            return None
        return self._row_to_api_key_view(row)

    async def get_api_key(self, key_id: str) -> ApiKeyView | None:
        """Look up an API key by ID."""
        async with self._pool.acquire() as conn:
            row = await conn.fetchrow(
                "SELECT key_id, user_id, name, key_prefix, key_hash, scopes, "
                "resource_limits, created_at, expires_at, revoked_at, grace_replaced_by "
                "FROM api_key_projection WHERE key_id = $1",
                key_id,
            )
        if row is None:
            return None
        return self._row_to_api_key_view(row)

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
        params: list[str | int | bool] = []
        idx = 1
        if user_id is not None:
            clauses.append(f"user_id = ${idx}")
            params.append(user_id)
            idx += 1
        if not include_revoked:
            # Include keys in grace period (revoked_at set to a future timestamp)
            clauses.append(f"(revoked_at IS NULL OR revoked_at > ${idx})")
            params.append(_now())
            idx += 1
        where = f" WHERE {' AND '.join(clauses)}" if clauses else ""
        params.extend([limit, offset])
        async with self._pool.acquire() as conn:
            rows = await conn.fetch(
                "SELECT key_id, user_id, name, key_prefix, key_hash, scopes, "
                "resource_limits, created_at, expires_at, revoked_at, grace_replaced_by "
                f"FROM api_key_projection{where} "
                f"ORDER BY created_at DESC LIMIT ${idx} OFFSET ${idx + 1}",
                *params,
            )
        return [self._row_to_api_key_view(r) for r in rows]

    async def revoke_api_key(self, key_id: str) -> None:
        """Set revoked_at to now on an API key."""
        async with self._pool.acquire() as conn:
            await conn.execute(
                "UPDATE api_key_projection SET revoked_at = $1 WHERE key_id = $2",
                _now(),
                key_id,
            )

    async def set_grace_replacement(
        self, key_id: str, *, replaced_by: str, revoked_at: str
    ) -> None:
        """Mark old key as replaced during rotation."""
        async with self._pool.acquire() as conn:
            await conn.execute(
                "UPDATE api_key_projection "
                "SET grace_replaced_by = $1, revoked_at = $2 WHERE key_id = $3",
                replaced_by,
                revoked_at,
                key_id,
            )

    # ── AuthStore: Resource limit queries ────────────────────────────────

    async def count_dispatches_since(self, user_id: str, since: str) -> int:
        """Count dispatches created by user since timestamp."""
        async with self._pool.acquire() as conn:
            row = await conn.fetchrow(
                "SELECT COUNT(*) FROM dispatch_projection WHERE user_id = $1 AND created_at >= $2",
                user_id,
                since,
            )
        return int(row[0]) if row else 0

    async def count_active_vms(self, user_id: str) -> int:
        """Count VMs currently active for user."""
        async with self._pool.acquire() as conn:
            row = await conn.fetchrow(
                "SELECT COUNT(DISTINCT sp1.dispatch_id) FROM step_projection sp1 "
                "JOIN dispatch_projection dp ON sp1.dispatch_id = dp.dispatch_id "
                "WHERE dp.user_id = $1 "
                "AND sp1.step_type = 'provision' AND sp1.status = 'completed' "
                "AND NOT EXISTS ("
                "  SELECT 1 FROM step_projection sp2 "
                "  WHERE sp2.dispatch_id = sp1.dispatch_id "
                "  AND sp2.step_type = 'teardown' AND sp2.status = 'completed'"
                ")",
                user_id,
            )
        return int(row[0]) if row else 0

    async def sum_cost_since(self, user_id: str, since: str) -> float:
        """Sum USD cost from TokenUsageRecorded events for user since timestamp."""
        async with self._pool.acquire() as conn:
            row = await conn.fetchrow(
                "SELECT COALESCE(SUM((payload->>'total_cost')::NUMERIC), 0) "
                "FROM events "
                "WHERE entity_type = 'dispatch' AND event_type = 'TokenUsageRecorded' "
                "AND entity_id IN (SELECT dispatch_id FROM dispatch_projection WHERE user_id = $1) "
                "AND timestamp >= $2",
                user_id,
                since,
            )
        return float(row[0]) if row else 0.0

    # ── Lifecycle ─────────────────────────────────────────────────────────

    async def close(self) -> None:
        """Close the pool if this store owns it."""
        if self._owns_pool:
            await self._pool.close()

    # ── Internal helpers ──────────────────────────────────────────────────

    @staticmethod
    def _row_to_dispatch_view(row: asyncpg.Record) -> DispatchView:
        dispatch_data = row["dispatch_json"]
        # asyncpg returns JSONB as dict; use model_validate for dicts
        if isinstance(dispatch_data, dict):
            dispatch = Dispatch.model_validate(dispatch_data)
        else:
            dispatch = Dispatch.model_validate_json(dispatch_data)
        return DispatchView(
            dispatch_id=str(row["dispatch_id"]),
            mode=DispatchMode(str(row["mode"])),
            status=DispatchStatus(str(row["status"])),
            outcome=Outcome(str(row["outcome"])) if row["outcome"] else None,
            lane=Lane(str(row["lane"])),
            preserve_on_failure=bool(row["preserve_on_failure"]),
            dispatch=dispatch,
            user_id=str(row["user_id"]) if row["user_id"] else "",
            created_at=str(row["created_at"]),
            updated_at=str(row["updated_at"]),
        )

    @staticmethod
    def _row_to_api_key_view(row: asyncpg.Record) -> ApiKeyView:
        scopes_raw = row["scopes"]
        if isinstance(scopes_raw, str):
            scopes = json.loads(scopes_raw)
        elif isinstance(scopes_raw, list):
            scopes = scopes_raw
        else:
            scopes = list(scopes_raw) if scopes_raw else []
        rl_raw = row["resource_limits"]
        resource_limits = None
        if rl_raw:
            if isinstance(rl_raw, dict):
                resource_limits = ResourceLimits.model_validate(rl_raw)
            else:
                resource_limits = ResourceLimits.model_validate_json(str(rl_raw))
        return ApiKeyView(
            key_id=str(row["key_id"]),
            user_id=str(row["user_id"]),
            name=str(row["name"]),
            key_prefix=str(row["key_prefix"]),
            key_hash=str(row["key_hash"]),
            scopes=scopes,
            resource_limits=resource_limits,
            created_at=str(row["created_at"]),
            expires_at=str(row["expires_at"]) if row["expires_at"] else None,
            revoked_at=str(row["revoked_at"]) if row["revoked_at"] else None,
            grace_replaced_by=str(row["grace_replaced_by"]) if row["grace_replaced_by"] else None,
        )

    @staticmethod
    def _row_to_step_view(row: asyncpg.Record) -> StepView:
        result_json = row["result_json"]
        # asyncpg returns JSONB as dict; serialise back to string for StepView
        if isinstance(result_json, dict):
            result_json = json.dumps(result_json)
        payload_json = row.get("payload_json")
        if isinstance(payload_json, dict):
            payload_json = json.dumps(payload_json)
        return StepView(
            step_id=str(row["step_id"]),
            dispatch_id=str(row["dispatch_id"]),
            step_type=StepType(str(row["step_type"])),
            step_sequence=int(row["step_sequence"]),
            lane=Lane(str(row["lane"])) if row["lane"] else None,
            status=StepStatus(str(row["status"])),
            worker_id=str(row["worker_id"]) if row["worker_id"] else None,
            result_json=result_json,
            error=str(row["error"]) if row["error"] else None,
            retry_count=int(row["retry_count"]),
            created_at=str(row["created_at"]),
            updated_at=str(row["updated_at"]),
        )
