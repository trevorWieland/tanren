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
from tanren_core.store.enums import DispatchMode, DispatchStatus, Lane, StepStatus, StepType
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
                "(event_id, timestamp, workflow_id, event_type, payload) "
                "VALUES ($1, $2, $3, $4, $5)",
                event_id,
                event.timestamp,
                event.workflow_id,
                event_type,
                payload,
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
        clauses: list[str] = []
        params: list[str | int] = []
        idx = 1

        if dispatch_id is not None:
            clauses.append(f"workflow_id = ${idx}")
            params.append(dispatch_id)
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
            "SELECT id, event_id, timestamp, workflow_id, "
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
                    workflow_id=r["workflow_id"],
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
                "VALUES ($1, $2, $3, $4, $5)",
                uuid.uuid4().hex,
                now,
                dispatch_id,
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

            # Select pending step, excluding cancelled dispatches
            cols = "s.step_id, s.dispatch_id, s.step_type, s.step_sequence, s.lane, s.payload_json"
            cancelled_filter = (
                "JOIN dispatch_projection d ON s.dispatch_id = d.dispatch_id "
                "WHERE s.status = 'pending' AND d.status != 'cancelled'"
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
                row = await conn.fetchrow(
                    f"SELECT {cols} FROM step_projection s "
                    f"{cancelled_filter} AND s.lane IS NULL "
                    "ORDER BY s.step_sequence, s.created_at "
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
                        "(event_id, timestamp, workflow_id, event_type, payload) "
                        "VALUES ($1, $2, $3, $4, $5)",
                        evt_id,
                        evt.timestamp,
                        evt.workflow_id,
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
                "VALUES ($1, $2, $3, $4, $5)",
                uuid.uuid4().hex,
                now,
                next_dispatch_id,
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
            "preserve_on_failure, dispatch_json, created_at, updated_at "
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
            "preserve_on_failure, dispatch_json, created_at, updated_at "
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
    ) -> None:
        """Insert a new dispatch projection row."""
        now = _now()
        async with self._pool.acquire() as conn, conn.transaction():
            await conn.execute(
                "INSERT INTO dispatch_projection "
                "(dispatch_id, mode, status, lane, "
                "preserve_on_failure, dispatch_json, "
                "created_at, updated_at) "
                "VALUES ($1, $2, 'pending', $3, $4, $5, $6, $7)",
                dispatch_id,
                str(mode),
                str(lane),
                preserve_on_failure,
                dispatch_json,
                now,
                now,
            )

    async def update_dispatch_status(
        self,
        dispatch_id: str,
        status: DispatchStatus,
        outcome: Outcome | None = None,
    ) -> None:
        """Update dispatch projection status."""
        now = _now()
        async with self._pool.acquire() as conn, conn.transaction():
            await conn.execute(
                "UPDATE dispatch_projection "
                "SET status = $1, outcome = $2, updated_at = $3 "
                "WHERE dispatch_id = $4",
                str(status),
                str(outcome) if outcome else None,
                now,
                dispatch_id,
            )

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
            created_at=str(row["created_at"]),
            updated_at=str(row["updated_at"]),
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
