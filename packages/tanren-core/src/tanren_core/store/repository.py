"""Unified async store — single implementation for SQLite and Postgres.

Uses SQLAlchemy 2.0 ORM queries with the engine/dialect handling backend
differences automatically.  Only ``dequeue()`` and JSON extraction have
small dialect-specific code paths.

Implements all four protocols: ``EventStore``, ``JobQueue``, ``StateStore``,
and ``AuthStore``.
"""

from __future__ import annotations

import asyncio
import json
import logging
import uuid
from collections.abc import AsyncIterator
from contextlib import asynccontextmanager
from datetime import UTC, datetime, timedelta

from sqlalchemy import Float, Numeric, func, or_, select, text, update
from sqlalchemy.ext.asyncio import AsyncEngine, AsyncSession, async_sessionmaker

from tanren_core.adapters.events import Event
from tanren_core.schemas import Outcome
from tanren_core.store.auth_views import ApiKeyView, UserView
from tanren_core.store.converters import (
    api_key_to_view,
    dispatch_to_view,
    event_to_record,
    step_to_queued,
    step_to_view,
    user_to_view,
)
from tanren_core.store.enums import DispatchMode, DispatchStatus, Lane, StepType
from tanren_core.store.events import StepEnqueued
from tanren_core.store.models import (
    ApiKeyProjection,
    DispatchProjection,
    EventModel,
    StepProjection,
    UserProjection,
)
from tanren_core.store.views import (
    DispatchListFilter,
    DispatchView,
    EventQueryResult,
    QueuedStep,
    StepView,
)
from tanren_core.timestamps import utc_now_iso

logger = logging.getLogger(__name__)


class Store:
    """Unified async store implementing EventStore, JobQueue, StateStore, AuthStore."""

    def __init__(
        self,
        session_factory: async_sessionmaker[AsyncSession],
        *,
        is_sqlite: bool,
        engine: AsyncEngine,
    ) -> None:
        """Initialize with a session factory and engine metadata.

        Args:
            session_factory: Async session factory bound to the engine.
            is_sqlite: True if the backend is SQLite (enables write serialization).
            engine: The underlying async engine (for close/dispose).
        """
        self._sf = session_factory
        self._is_sqlite = is_sqlite
        self._engine = engine
        self._sqlite_lock = asyncio.Lock() if is_sqlite else None

    # ── EventStore ────────────────────────────────────────────────────────

    async def append(self, event: Event) -> None:
        """Append an event to the log."""
        async with self._write_session() as session:
            session.add(
                EventModel(
                    event_id=uuid.uuid4().hex,
                    timestamp=event.timestamp,
                    entity_id=event.entity_id,
                    entity_type=str(event.entity_type),
                    event_type=type(event).__name__,
                    payload=event.model_dump(mode="json"),
                )
            )

    async def query_events(
        self,
        *,
        entity_id: str | None = None,
        entity_ids: list[str] | None = None,
        entity_type: str | None = None,
        event_type: str | None = None,
        since: str | None = None,
        until: str | None = None,
        owner_user_id: str | None = None,
        owner_key_id: str | None = None,
        limit: int = 50,
        offset: int = 0,
    ) -> EventQueryResult:
        """Query events with optional filters and pagination."""
        async with self._sf() as session:
            base = select(EventModel)
            count_base = select(func.count()).select_from(EventModel)

            conditions = []
            if entity_id is not None:
                conditions.append(EventModel.entity_id == entity_id)
            if entity_ids is not None:
                conditions.append(EventModel.entity_id.in_(entity_ids))

            # DB-level ownership filter: events for the user's dispatches,
            # or events where entity_id is the user/key ID directly.
            if owner_user_id is not None:
                user_dispatches = select(DispatchProjection.dispatch_id).where(
                    DispatchProjection.user_id == owner_user_id
                )
                ownership_clauses = [
                    EventModel.entity_id.in_(user_dispatches),
                    EventModel.entity_id == owner_user_id,
                ]
                if owner_key_id is not None:
                    ownership_clauses = [
                        *ownership_clauses,
                        EventModel.entity_id == owner_key_id,
                    ]
                conditions.append(or_(*ownership_clauses))
            if entity_type is not None:
                conditions.append(EventModel.entity_type == entity_type)
            if event_type is not None:
                conditions.append(EventModel.event_type == event_type)
            if since is not None:
                conditions.append(EventModel.timestamp >= since)
            if until is not None:
                conditions.append(EventModel.timestamp <= until)

            for cond in conditions:
                base = base.where(cond)
                count_base = count_base.where(cond)

            total = (await session.execute(count_base)).scalar_one()
            rows = (
                (
                    await session.execute(
                        base.order_by(EventModel.id.desc()).offset(offset).limit(limit)
                    )
                )
                .scalars()
                .all()
            )

            return EventQueryResult(
                events=[event_to_record(r) for r in rows],
                total=total,
            )

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
        """Insert a new step into the queue."""
        now = utc_now_iso()
        payload_dict = json.loads(payload_json)

        async with self._write_session() as session:
            # Insert step projection
            session.add(
                StepProjection(
                    step_id=step_id,
                    dispatch_id=dispatch_id,
                    step_type=step_type,
                    step_sequence=step_sequence,
                    lane=lane,
                    status="pending",
                    payload_json=payload_dict,
                    retry_count=0,
                    created_at=now,
                    updated_at=now,
                )
            )
            # Append StepEnqueued event

            evt = StepEnqueued(
                timestamp=now,
                entity_id=dispatch_id,
                step_id=step_id,
                step_type=StepType(step_type),
                step_sequence=step_sequence,
                lane=Lane(lane) if lane else None,
            )
            session.add(
                EventModel(
                    event_id=uuid.uuid4().hex,
                    timestamp=now,
                    entity_id=dispatch_id,
                    entity_type=str(evt.entity_type),
                    event_type="StepEnqueued",
                    payload=evt.model_dump(mode="json"),
                )
            )
            # Update dispatch status to running (if still pending)
            await session.execute(
                update(DispatchProjection)
                .where(DispatchProjection.dispatch_id == dispatch_id)
                .where(DispatchProjection.status == "pending")
                .values(status="running", updated_at=now)
            )

    async def dequeue(
        self,
        *,
        lane: Lane | None = None,
        worker_id: str,
        max_concurrent: int,
    ) -> QueuedStep | None:
        """Atomically claim a pending step if capacity allows."""
        if self._is_sqlite:
            return await self._dequeue_sqlite(
                lane=lane, worker_id=worker_id, max_concurrent=max_concurrent
            )
        return await self._dequeue_postgres(
            lane=lane, worker_id=worker_id, max_concurrent=max_concurrent
        )

    async def ack(self, step_id: str, *, result_json: str) -> None:
        """Mark a step as completed and store its result."""
        now = utc_now_iso()
        result_dict = json.loads(result_json)
        async with self._write_session() as session:
            await session.execute(
                update(StepProjection)
                .where(StepProjection.step_id == step_id)
                .values(status="completed", result_json=result_dict, error=None, updated_at=now)
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
        """Atomically ack a step and enqueue the next step."""
        now = utc_now_iso()
        result_dict = json.loads(result_json)
        next_payload_dict = json.loads(next_payload_json)

        async with self._write_session() as session:
            # 1. Ack current step
            await session.execute(
                update(StepProjection)
                .where(StepProjection.step_id == step_id)
                .values(status="completed", result_json=result_dict, error=None, updated_at=now)
            )

            # 2. Insert completion events
            if completion_events:
                for evt in completion_events:
                    session.add(
                        EventModel(
                            event_id=uuid.uuid4().hex,
                            timestamp=evt.timestamp,
                            entity_id=evt.entity_id,
                            entity_type=str(evt.entity_type),
                            event_type=type(evt).__name__,
                            payload=evt.model_dump(mode="json"),
                        )
                    )

            # 3. Insert next step
            session.add(
                StepProjection(
                    step_id=next_step_id,
                    dispatch_id=next_dispatch_id,
                    step_type=next_step_type,
                    step_sequence=next_step_sequence,
                    lane=next_lane,
                    status="pending",
                    payload_json=next_payload_dict,
                    retry_count=0,
                    created_at=now,
                    updated_at=now,
                )
            )

            # 4. Append StepEnqueued event

            enq_evt = StepEnqueued(
                timestamp=now,
                entity_id=next_dispatch_id,
                step_id=next_step_id,
                step_type=StepType(next_step_type),
                step_sequence=next_step_sequence,
                lane=Lane(next_lane) if next_lane else None,
            )
            session.add(
                EventModel(
                    event_id=uuid.uuid4().hex,
                    timestamp=now,
                    entity_id=next_dispatch_id,
                    entity_type=str(enq_evt.entity_type),
                    event_type="StepEnqueued",
                    payload=enq_evt.model_dump(mode="json"),
                )
            )

            # 5. Update dispatch status (if still pending)
            await session.execute(
                update(DispatchProjection)
                .where(DispatchProjection.dispatch_id == next_dispatch_id)
                .where(DispatchProjection.status == "pending")
                .values(status="running", updated_at=now)
            )

    async def cancel_pending_steps(self, dispatch_id: str) -> int:
        """Cancel pending forward-progress steps (teardowns preserved)."""
        now = utc_now_iso()
        async with self._write_session() as session:
            result = await session.execute(
                update(StepProjection)
                .where(StepProjection.dispatch_id == dispatch_id)
                .where(StepProjection.status == "pending")
                .where(StepProjection.step_type != "teardown")
                .values(status="cancelled", updated_at=now)
            )
            return getattr(result, "rowcount", 0) or 0

    async def nack(
        self,
        step_id: str,
        *,
        error: str,
        error_class: str | None = None,  # noqa: ARG002 — protocol signature
        retry: bool = False,
    ) -> None:
        """Mark a step as failed, optionally re-enqueuing for retry."""
        now = utc_now_iso()
        async with self._write_session() as session:
            if retry:
                await session.execute(
                    update(StepProjection)
                    .where(StepProjection.step_id == step_id)
                    .values(
                        status="pending",
                        error=error,
                        retry_count=StepProjection.retry_count + 1,
                        worker_id=None,
                        updated_at=now,
                    )
                )
            else:
                await session.execute(
                    update(StepProjection)
                    .where(StepProjection.step_id == step_id)
                    .values(status="failed", error=error, updated_at=now)
                )

    async def recover_stale_steps(self, *, timeout_secs: int = 300) -> int:
        """Reset running steps older than timeout back to pending."""
        now = utc_now_iso()
        cutoff = (
            (datetime.now(UTC) - timedelta(seconds=timeout_secs)).isoformat().replace("+00:00", "Z")
        )

        async with self._write_session() as session:
            result = await session.execute(
                update(StepProjection)
                .where(StepProjection.status == "running")
                .where(StepProjection.updated_at < cutoff)
                .values(status="pending", worker_id=None, updated_at=now)
            )
            return getattr(result, "rowcount", 0) or 0

    # ── StateStore ────────────────────────────────────────────────────────

    async def get_dispatch(self, dispatch_id: str) -> DispatchView | None:
        """Look up a dispatch by ID."""
        async with self._sf() as session:
            row = await session.get(DispatchProjection, dispatch_id)
            return dispatch_to_view(row) if row else None

    async def query_dispatches(self, filters: DispatchListFilter) -> list[DispatchView]:
        """Query dispatches with filters and pagination."""
        async with self._sf() as session:
            stmt = select(DispatchProjection)

            if filters.status is not None:
                stmt = stmt.where(DispatchProjection.status == str(filters.status))
            if filters.lane is not None:
                stmt = stmt.where(DispatchProjection.lane == str(filters.lane))
            if filters.project is not None:
                stmt = stmt.where(
                    DispatchProjection.dispatch_json["project"].as_string() == filters.project
                )
            if filters.user_id is not None:
                stmt = stmt.where(DispatchProjection.user_id == filters.user_id)
            if filters.since is not None:
                stmt = stmt.where(DispatchProjection.created_at >= filters.since)
            if filters.until is not None:
                stmt = stmt.where(DispatchProjection.created_at <= filters.until)

            stmt = stmt.order_by(DispatchProjection.created_at.desc())
            stmt = stmt.offset(filters.offset).limit(filters.limit)

            rows = (await session.execute(stmt)).scalars().all()
            return [dispatch_to_view(r) for r in rows]

    async def get_step(self, step_id: str) -> StepView | None:
        """Look up a step by ID."""
        async with self._sf() as session:
            row = await session.get(StepProjection, step_id)
            return step_to_view(row) if row else None

    async def get_steps_for_dispatch(self, dispatch_id: str) -> list[StepView]:
        """Get all steps for a dispatch, ordered by step_sequence."""
        async with self._sf() as session:
            rows = (
                (
                    await session.execute(
                        select(StepProjection)
                        .where(StepProjection.dispatch_id == dispatch_id)
                        .order_by(StepProjection.step_sequence)
                    )
                )
                .scalars()
                .all()
            )
            return [step_to_view(r) for r in rows]

    async def count_running_steps(self, *, lane: Lane | None = None) -> int:
        """Count steps with status='running' for the given lane."""
        async with self._sf() as session:
            stmt = (
                select(func.count())
                .select_from(StepProjection)
                .where(StepProjection.status == "running")
            )
            if lane is not None:
                stmt = stmt.where(StepProjection.lane == str(lane))
            else:
                stmt = stmt.where(StepProjection.lane.is_(None))
            return (await session.execute(stmt)).scalar_one()

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
        now = utc_now_iso()
        dispatch_dict = json.loads(dispatch_json)
        async with self._write_session() as session:
            session.add(
                DispatchProjection(
                    dispatch_id=dispatch_id,
                    mode=str(mode),
                    status="pending",
                    lane=str(lane),
                    preserve_on_failure=preserve_on_failure,
                    dispatch_json=dispatch_dict,
                    user_id=user_id,
                    created_at=now,
                    updated_at=now,
                )
            )

    async def update_dispatch_status(
        self,
        dispatch_id: str,
        status: DispatchStatus,
        outcome: Outcome | None = None,
    ) -> None:
        """Update dispatch status (silently ignores terminal states)."""
        now = utc_now_iso()
        terminal = {"completed", "failed", "cancelled"}
        async with self._write_session() as session:
            values: dict = {"status": str(status), "updated_at": now}
            if outcome is not None:
                values["outcome"] = str(outcome)
            await session.execute(
                update(DispatchProjection)
                .where(DispatchProjection.dispatch_id == dispatch_id)
                .where(DispatchProjection.status.notin_(terminal))
                .values(**values)
            )

    # ── AuthStore ─────────────────────────────────────────────────────────

    async def create_user(
        self,
        *,
        user_id: str,
        name: str,
        email: str | None,
        role: str,
    ) -> None:
        """Create a user projection."""
        now = utc_now_iso()
        async with self._write_session() as session:
            session.add(
                UserProjection(
                    user_id=user_id,
                    name=name,
                    email=email,
                    role=role,
                    is_active=True,
                    created_at=now,
                    updated_at=now,
                )
            )

    async def get_user(self, user_id: str) -> UserView | None:
        """Look up a user by ID."""
        async with self._sf() as session:
            row = await session.get(UserProjection, user_id)
            return user_to_view(row) if row else None

    async def list_users(self, *, limit: int = 50, offset: int = 0) -> list[UserView]:
        """List users with pagination."""
        async with self._sf() as session:
            rows = (
                (
                    await session.execute(
                        select(UserProjection)
                        .order_by(UserProjection.created_at.desc())
                        .offset(offset)
                        .limit(limit)
                    )
                )
                .scalars()
                .all()
            )
            return [user_to_view(r) for r in rows]

    async def update_user(
        self,
        user_id: str,
        *,
        name: str | None = None,
        email: str | None = None,
        role: str | None = None,
    ) -> None:
        """Update user fields."""
        now = utc_now_iso()
        values: dict = {"updated_at": now}
        if name is not None:
            values["name"] = name
        if email is not None:
            values["email"] = email
        if role is not None:
            values["role"] = role
        async with self._write_session() as session:
            await session.execute(
                update(UserProjection).where(UserProjection.user_id == user_id).values(**values)
            )

    async def deactivate_user(self, user_id: str) -> None:
        """Deactivate a user."""
        now = utc_now_iso()
        async with self._write_session() as session:
            await session.execute(
                update(UserProjection)
                .where(UserProjection.user_id == user_id)
                .values(is_active=False, updated_at=now)
            )

    async def create_api_key(
        self,
        *,
        key_id: str,
        user_id: str,
        name: str,
        key_prefix: str,
        key_hash: str,
        scopes_json: str,
        resource_limits_json: str | None = None,
        expires_at: str | None = None,
    ) -> None:
        """Create an API key projection."""
        now = utc_now_iso()
        scopes_list = json.loads(scopes_json)
        rl_dict = json.loads(resource_limits_json) if resource_limits_json else None
        async with self._write_session() as session:
            session.add(
                ApiKeyProjection(
                    key_id=key_id,
                    user_id=user_id,
                    name=name,
                    key_prefix=key_prefix,
                    key_hash=key_hash,
                    scopes=scopes_list,
                    resource_limits=rl_dict,
                    created_at=now,
                    expires_at=expires_at,
                )
            )

    async def get_api_key_by_hash(self, key_hash: str) -> ApiKeyView | None:
        """Look up an API key by its SHA-256 hash."""
        async with self._sf() as session:
            row = (
                (
                    await session.execute(
                        select(ApiKeyProjection).where(ApiKeyProjection.key_hash == key_hash)
                    )
                )
                .scalars()
                .first()
            )
            return api_key_to_view(row) if row else None

    async def get_api_key(self, key_id: str) -> ApiKeyView | None:
        """Look up an API key by ID."""
        async with self._sf() as session:
            row = await session.get(ApiKeyProjection, key_id)
            return api_key_to_view(row) if row else None

    async def list_api_keys(
        self,
        *,
        user_id: str | None = None,
        include_revoked: bool = False,
        limit: int = 50,
        offset: int = 0,
    ) -> list[ApiKeyView]:
        """List API keys with optional filtering."""
        now = utc_now_iso()
        async with self._sf() as session:
            stmt = select(ApiKeyProjection)
            if user_id is not None:
                stmt = stmt.where(ApiKeyProjection.user_id == user_id)
            if not include_revoked:
                # Include keys in grace period (revoked_at in the future)
                stmt = stmt.where(
                    (ApiKeyProjection.revoked_at.is_(None)) | (ApiKeyProjection.revoked_at > now)
                )
            stmt = stmt.order_by(ApiKeyProjection.created_at.desc()).offset(offset).limit(limit)
            rows = (await session.execute(stmt)).scalars().all()
            return [api_key_to_view(r) for r in rows]

    async def revoke_api_key(self, key_id: str) -> None:
        """Revoke an API key by setting revoked_at to now."""
        now = utc_now_iso()
        async with self._write_session() as session:
            await session.execute(
                update(ApiKeyProjection)
                .where(ApiKeyProjection.key_id == key_id)
                .values(revoked_at=now)
            )

    async def set_grace_replacement(
        self, key_id: str, *, replaced_by: str, revoked_at: str
    ) -> None:
        """Mark a key as replaced during rotation."""
        async with self._write_session() as session:
            await session.execute(
                update(ApiKeyProjection)
                .where(ApiKeyProjection.key_id == key_id)
                .values(grace_replaced_by=replaced_by, revoked_at=revoked_at)
            )

    # ── Resource limit queries ────────────────────────────────────────────

    async def count_dispatches_since(self, user_id: str, since: str) -> int:
        """Count dispatches created by user since timestamp."""
        async with self._sf() as session:
            return (
                await session.execute(
                    select(func.count())
                    .select_from(DispatchProjection)
                    .where(DispatchProjection.user_id == user_id)
                    .where(DispatchProjection.created_at >= since)
                )
            ).scalar_one()

    async def count_active_vms(self, user_id: str) -> int:
        """Count active VMs (provisioned but not torn down) for a user."""
        async with self._sf() as session:
            return (
                await session.execute(
                    select(func.count(func.distinct(StepProjection.dispatch_id)))
                    .join(
                        DispatchProjection,
                        StepProjection.dispatch_id == DispatchProjection.dispatch_id,
                    )
                    .where(DispatchProjection.user_id == user_id)
                    .where(StepProjection.step_type == "provision")
                    .where(StepProjection.status.in_(["pending", "running", "completed"]))
                    .where(
                        ~StepProjection.dispatch_id.in_(
                            select(StepProjection.dispatch_id)
                            .where(StepProjection.step_type == "teardown")
                            .where(StepProjection.status == "completed")
                        )
                    )
                )
            ).scalar_one()

    async def sum_cost_since(self, user_id: str, since: str) -> float:
        """Sum token usage costs for a user since timestamp."""
        async with self._sf() as session:
            # Dialect-specific JSON extraction
            if self._is_sqlite:
                cost_expr = func.cast(func.json_extract(EventModel.payload, "$.total_cost"), Float)
            else:
                cost_expr = func.cast(EventModel.payload["total_cost"].as_string(), Numeric)

            user_dispatches = select(DispatchProjection.dispatch_id).where(
                DispatchProjection.user_id == user_id
            )

            result = (
                await session.execute(
                    select(func.coalesce(func.sum(cost_expr), 0.0))
                    .select_from(EventModel)
                    .where(EventModel.entity_type == "dispatch")
                    .where(EventModel.event_type == "TokenUsageRecorded")
                    .where(EventModel.entity_id.in_(user_dispatches))
                    .where(EventModel.timestamp >= since)
                )
            ).scalar_one()
            return float(result)

    # ── Quota locking ─────────────────────────────────────────────────────

    @asynccontextmanager
    async def user_quota_lock(self, user_id: str) -> AsyncIterator[None]:
        """Serialize limit-checked operations for a user.

        Prevents TOCTOU races where concurrent requests both pass the
        quota check before either creates.  Uses ``pg_advisory_xact_lock``
        on Postgres (per-user, released on session close).  On SQLite,
        this is a no-op because the single-writer ``_sqlite_lock`` already
        serializes all write operations.

        Yields:
            None — use as ``async with store.user_quota_lock(uid): ...``.
        """
        if self._is_sqlite:
            # SQLite: single writer serialized by _write_session lock — no-op here
            yield
        else:
            # Postgres: per-user session-scoped advisory lock
            async with self._sf() as session:
                await session.execute(
                    text("SELECT pg_advisory_lock(hashtext(:key))"),
                    {"key": f"quota-{user_id}"},
                )
                try:
                    yield
                finally:
                    await session.execute(
                        text("SELECT pg_advisory_unlock(hashtext(:key))"),
                        {"key": f"quota-{user_id}"},
                    )

    # ── Lifecycle ─────────────────────────────────────────────────────────

    async def close(self) -> None:
        """Dispose the engine and release all connections."""
        await self._engine.dispose()

    # ── Private helpers ───────────────────────────────────────────────────

    @asynccontextmanager
    async def _write_session(self) -> AsyncIterator[AsyncSession]:
        """Context manager for write transactions.

        On SQLite, acquires the serialization lock first to prevent
        concurrent writes (replaces ``BEGIN IMMEDIATE``).

        Yields:
            An ``AsyncSession`` inside an active transaction.
        """
        if self._sqlite_lock is not None:
            async with self._sqlite_lock, self._sf.begin() as session:
                yield session
        else:
            async with self._sf.begin() as session:
                yield session

    # ── Dequeue implementations ───────────────────────────────────────────

    async def _dequeue_sqlite(
        self,
        *,
        lane: Lane | None,
        worker_id: str,
        max_concurrent: int,
    ) -> QueuedStep | None:
        """SQLite dequeue with asyncio.Lock serialization.

        Raises:
            RuntimeError: If SQLite lock is not initialized.
        """
        if self._sqlite_lock is None:
            msg = "SQLite lock not initialized"
            raise RuntimeError(msg)
        async with self._sqlite_lock, self._sf.begin() as session:
            return await self._dequeue_core(
                session, lane=lane, worker_id=worker_id, max_concurrent=max_concurrent
            )

    async def _dequeue_postgres(
        self,
        *,
        lane: Lane | None,
        worker_id: str,
        max_concurrent: int,
    ) -> QueuedStep | None:
        """Postgres dequeue with advisory lock + FOR UPDATE SKIP LOCKED."""
        async with self._sf.begin() as session:
            # Per-lane advisory lock
            lane_key = f"dequeue-{lane}" if lane is not None else "dequeue-infra"
            await session.execute(
                text("SELECT pg_advisory_xact_lock(hashtext(:key))"), {"key": lane_key}
            )
            return await self._dequeue_core(
                session,
                lane=lane,
                worker_id=worker_id,
                max_concurrent=max_concurrent,
                use_for_update=True,
            )

    async def _dequeue_core(
        self,
        session: AsyncSession,
        *,
        lane: Lane | None,
        worker_id: str,
        max_concurrent: int,
        use_for_update: bool = False,
    ) -> QueuedStep | None:
        """Shared dequeue logic — capacity check, step selection, claim."""
        # 1. Count running steps for this lane
        count_stmt = (
            select(func.count())
            .select_from(StepProjection)
            .where(StepProjection.status == "running")
        )
        if lane is not None:
            count_stmt = count_stmt.where(StepProjection.lane == str(lane))
        else:
            count_stmt = count_stmt.where(StepProjection.lane.is_(None))

        running = (await session.execute(count_stmt)).scalar_one()
        if running >= max_concurrent:
            return None

        # 2. Select next pending step (skip cancelled non-teardown dispatches)
        step_stmt = (
            select(StepProjection)
            .join(
                DispatchProjection,
                StepProjection.dispatch_id == DispatchProjection.dispatch_id,
            )
            .where(StepProjection.status == "pending")
            .where(
                (DispatchProjection.status != "cancelled")
                | (StepProjection.step_type == "teardown")
            )
        )

        if lane is not None:
            step_stmt = step_stmt.where(StepProjection.lane == str(lane))
            step_stmt = step_stmt.order_by(StepProjection.step_sequence, StepProjection.created_at)
        else:
            step_stmt = step_stmt.where(StepProjection.lane.is_(None))
            # Infra lane: FIFO by created_at to prevent teardown starvation
            step_stmt = step_stmt.order_by(StepProjection.created_at, StepProjection.step_sequence)

        step_stmt = step_stmt.limit(1)

        if use_for_update:
            step_stmt = step_stmt.with_for_update(skip_locked=True)

        row = (await session.execute(step_stmt)).scalars().first()
        if row is None:
            return None

        # 3. Claim the step
        now = utc_now_iso()
        await session.execute(
            update(StepProjection)
            .where(StepProjection.step_id == row.step_id)
            .values(status="running", worker_id=worker_id, updated_at=now)
        )

        return step_to_queued(row)
