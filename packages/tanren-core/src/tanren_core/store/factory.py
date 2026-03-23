"""Factory for creating store backends from a database URL."""

from __future__ import annotations

import asyncpg

from tanren_core.adapters.postgres_pool import is_postgres_url
from tanren_core.store.postgres import PostgresStore
from tanren_core.store.sqlite import SqliteStore


async def create_sqlite_store(db_path: str) -> SqliteStore:
    """Create a SQLite-backed store.

    Returns:
        A ``SqliteStore`` instance (implements EventStore, JobQueue, StateStore).
    """
    store = SqliteStore(db_path)
    await store._ensure_conn()
    return store


async def create_postgres_store(dsn: str) -> PostgresStore:
    """Create a Postgres-backed store.

    Creates a fresh asyncpg pool and initialises the store schema tables.

    Returns:
        A ``PostgresStore`` instance (implements EventStore, JobQueue, StateStore).
    """
    pool = await asyncpg.create_pool(dsn, min_size=2, max_size=10)
    store = PostgresStore(pool, owns_pool=True)
    await store.ensure_schema()
    return store


async def create_store(
    db_url: str,
) -> SqliteStore | PostgresStore:
    """Create a store from a database URL.

    For SQLite paths, returns a ``SqliteStore``.
    For PostgreSQL URLs, returns a ``PostgresStore``.

    Returns:
        A store instance implementing EventStore, JobQueue, and StateStore.
    """
    if is_postgres_url(db_url):
        return await create_postgres_store(db_url)
    return await create_sqlite_store(db_url)
