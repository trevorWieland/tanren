"""Shared asyncpg pool factory and schema initialization."""

from __future__ import annotations

import logging

import asyncpg

logger = logging.getLogger(__name__)

_EVENTS_SCHEMA = """\
CREATE TABLE IF NOT EXISTS events (
    id BIGSERIAL PRIMARY KEY,
    timestamp TEXT NOT NULL,
    workflow_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_events_workflow ON events(workflow_id);
CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);
CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events(timestamp);
"""

_VM_STATE_SCHEMA = """\
CREATE TABLE IF NOT EXISTS vm_assignments (
    vm_id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    project TEXT NOT NULL,
    spec TEXT NOT NULL,
    host TEXT NOT NULL,
    assigned_at TEXT NOT NULL,
    released_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_vm_active
    ON vm_assignments(released_at) WHERE released_at IS NULL;
"""


def is_postgres_url(url: str | None) -> bool:
    """Return True if *url* looks like a Postgres DSN."""
    if not url:
        return False
    return url.lower().startswith(("postgresql://", "postgres://"))


async def ensure_schema(pool: asyncpg.Pool) -> None:
    """Create tables and indexes idempotently."""
    async with pool.acquire() as conn:
        for statement in _EVENTS_SCHEMA.strip().split(";"):
            statement = statement.strip()
            if statement:
                await conn.execute(statement)
        for statement in _VM_STATE_SCHEMA.strip().split(";"):
            statement = statement.strip()
            if statement:
                await conn.execute(statement)


async def create_postgres_pool(
    dsn: str,
    *,
    min_size: int = 2,
    max_size: int = 10,
) -> asyncpg.Pool:
    """Create an asyncpg pool and ensure the schema exists.

    Args:
        dsn: PostgreSQL connection string.
        min_size: Minimum pool connections.
        max_size: Maximum pool connections.

    Returns:
        Initialized asyncpg.Pool.
    """
    pool = await asyncpg.create_pool(dsn, min_size=min_size, max_size=max_size)
    await ensure_schema(pool)
    logger.info("Postgres pool created (%d-%d connections)", min_size, max_size)
    return pool
