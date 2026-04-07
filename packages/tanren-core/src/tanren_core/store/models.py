"""SQLAlchemy 2.0 ORM models for the event-sourced store.

These mapped classes define the single source of truth for the database
schema.  SQLAlchemy's type system adapts automatically between backends:

- ``JSON`` renders as TEXT on SQLite and JSONB on Postgres.
- ``Boolean`` renders as INTEGER on SQLite and BOOLEAN on Postgres.
- ``BigInteger`` renders as INTEGER on SQLite and BIGINT on Postgres.

Alembic auto-generates migrations from these models.  Domain views
(``DispatchView``, ``UserView``, etc.) remain as the public contract;
converter functions in ``converters.py`` bridge ORM rows to domain types.
"""

from __future__ import annotations

from typing import Any

from sqlalchemy import BigInteger, Boolean, ForeignKey, Index, Integer, String, Text, text
from sqlalchemy.orm import DeclarativeBase, Mapped, mapped_column
from sqlalchemy.types import JSON


class Base(DeclarativeBase):
    """Declarative base for all store ORM models."""


# ---------------------------------------------------------------------------
# events
# ---------------------------------------------------------------------------


class EventModel(Base):
    """Append-only event log."""

    __tablename__ = "events"

    id: Mapped[int] = mapped_column(
        BigInteger().with_variant(Integer, "sqlite"),
        primary_key=True,
        autoincrement=True,
    )
    event_id: Mapped[str] = mapped_column(String, unique=True, nullable=False)
    timestamp: Mapped[str] = mapped_column(String, nullable=False)
    entity_id: Mapped[str] = mapped_column(String, nullable=False)
    entity_type: Mapped[str] = mapped_column(String, nullable=False, server_default="dispatch")
    event_type: Mapped[str] = mapped_column(String, nullable=False)
    payload: Mapped[dict[str, Any]] = mapped_column(JSON, nullable=False)

    __table_args__ = (
        Index("idx_events_entity", "entity_id"),
        Index("idx_events_entity_type", "entity_type"),
        Index("idx_events_type", "event_type"),
        Index("idx_events_timestamp", "timestamp"),
    )


# ---------------------------------------------------------------------------
# dispatch_projection
# ---------------------------------------------------------------------------


class DispatchProjection(Base):
    """Materialized view of dispatch state."""

    __tablename__ = "dispatch_projection"

    dispatch_id: Mapped[str] = mapped_column(String, primary_key=True)
    mode: Mapped[str] = mapped_column(String, nullable=False)
    status: Mapped[str] = mapped_column(String, nullable=False, server_default="pending")
    outcome: Mapped[str | None] = mapped_column(String, nullable=True)
    lane: Mapped[str] = mapped_column(String, nullable=False)
    preserve_on_failure: Mapped[bool] = mapped_column(Boolean, nullable=False, server_default="0")
    dispatch_json: Mapped[dict[str, Any]] = mapped_column(JSON, nullable=False)
    user_id: Mapped[str] = mapped_column(String, nullable=False, server_default="")
    created_at: Mapped[str] = mapped_column(String, nullable=False)
    updated_at: Mapped[str] = mapped_column(String, nullable=False)

    __table_args__ = (
        Index("idx_dispatch_status", "status"),
        Index("idx_dispatch_lane", "lane"),
        Index("idx_dispatch_created", "created_at"),
        Index("idx_dispatch_user", "user_id"),
    )


# ---------------------------------------------------------------------------
# step_projection
# ---------------------------------------------------------------------------


class StepProjection(Base):
    """Job queue backing store — one row per lifecycle step."""

    __tablename__ = "step_projection"

    step_id: Mapped[str] = mapped_column(String, primary_key=True)
    dispatch_id: Mapped[str] = mapped_column(
        String, ForeignKey("dispatch_projection.dispatch_id"), nullable=False
    )
    step_type: Mapped[str] = mapped_column(String, nullable=False)
    step_sequence: Mapped[int] = mapped_column(nullable=False)
    lane: Mapped[str | None] = mapped_column(String, nullable=True)
    status: Mapped[str] = mapped_column(String, nullable=False, server_default="pending")
    worker_id: Mapped[str | None] = mapped_column(String, nullable=True)
    payload_json: Mapped[dict[str, Any]] = mapped_column(JSON, nullable=False)
    result_json: Mapped[dict[str, Any] | None] = mapped_column(JSON, nullable=True)
    error: Mapped[str | None] = mapped_column(Text, nullable=True)
    retry_count: Mapped[int] = mapped_column(nullable=False, server_default="0")
    created_at: Mapped[str] = mapped_column(String, nullable=False)
    updated_at: Mapped[str] = mapped_column(String, nullable=False)

    __table_args__ = (
        Index("idx_step_dispatch", "dispatch_id"),
        Index("idx_step_status", "status"),
        Index("idx_step_lane_status", "lane", "status"),
    )


# ---------------------------------------------------------------------------
# vm_assignments
# ---------------------------------------------------------------------------


class VMAssignment(Base):
    """VM lifecycle tracking — assignment and release."""

    __tablename__ = "vm_assignments"

    vm_id: Mapped[str] = mapped_column(String, primary_key=True)
    workflow_id: Mapped[str] = mapped_column(String, nullable=False)
    project: Mapped[str] = mapped_column(String, nullable=False)
    spec: Mapped[str] = mapped_column(String, nullable=False)
    host: Mapped[str] = mapped_column(String, nullable=False)
    assigned_at: Mapped[str] = mapped_column(String, nullable=False)
    released_at: Mapped[str | None] = mapped_column(String, nullable=True)

    __table_args__ = (
        Index(
            "idx_vm_active",
            "released_at",
            sqlite_where=text("released_at IS NULL"),
            postgresql_where=text("released_at IS NULL"),
        ),
    )


# ---------------------------------------------------------------------------
# user_projection
# ---------------------------------------------------------------------------


class UserProjection(Base):
    """User account projection."""

    __tablename__ = "user_projection"

    user_id: Mapped[str] = mapped_column(String, primary_key=True)
    name: Mapped[str] = mapped_column(String, nullable=False)
    email: Mapped[str | None] = mapped_column(String, nullable=True)
    role: Mapped[str] = mapped_column(String, nullable=False, server_default="member")
    is_active: Mapped[bool] = mapped_column(Boolean, nullable=False, server_default="1")
    created_at: Mapped[str] = mapped_column(String, nullable=False)
    updated_at: Mapped[str] = mapped_column(String, nullable=False)


# ---------------------------------------------------------------------------
# api_key_projection
# ---------------------------------------------------------------------------


class ApiKeyProjection(Base):
    """API key projection with scopes and resource limits."""

    __tablename__ = "api_key_projection"

    key_id: Mapped[str] = mapped_column(String, primary_key=True)
    user_id: Mapped[str] = mapped_column(
        String, ForeignKey("user_projection.user_id"), nullable=False
    )
    name: Mapped[str] = mapped_column(String, nullable=False)
    key_prefix: Mapped[str] = mapped_column(String, nullable=False)
    key_hash: Mapped[str] = mapped_column(String, unique=True, nullable=False)
    scopes: Mapped[list[str]] = mapped_column(JSON, nullable=False)
    resource_limits: Mapped[dict[str, Any] | None] = mapped_column(JSON, nullable=True)
    created_at: Mapped[str] = mapped_column(String, nullable=False)
    expires_at: Mapped[str | None] = mapped_column(String, nullable=True)
    revoked_at: Mapped[str | None] = mapped_column(String, nullable=True)
    grace_replaced_by: Mapped[str | None] = mapped_column(String, nullable=True)

    __table_args__ = (
        Index("idx_key_hash", "key_hash", unique=True),
        Index("idx_key_user", "user_id"),
        Index("idx_key_prefix", "key_prefix"),
    )
