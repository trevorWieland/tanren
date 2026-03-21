"""Factory for creating store backends from a database URL."""

from __future__ import annotations

from tanren_core.adapters.postgres_pool import is_postgres_url
from tanren_core.store.sqlite import SqliteStore


async def create_sqlite_store(db_path: str) -> SqliteStore:
    """Create a SQLite-backed store.

    Returns:
        A ``SqliteStore`` instance (implements EventStore, JobQueue, StateStore).
    """
    store = SqliteStore(db_path)
    await store._ensure_conn()
    return store


async def create_store(
    db_url: str,
) -> SqliteStore:
    """Create a store from a database URL.

    For SQLite paths, returns a ``SqliteStore``.
    For PostgreSQL URLs, raises ``NotImplementedError`` (Phase 2 follow-up).

    Returns:
        A store instance implementing EventStore, JobQueue, and StateStore.
    """
    if is_postgres_url(db_url):
        raise NotImplementedError("PostgresStore not yet implemented")
    return await create_sqlite_store(db_url)
