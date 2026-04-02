"""Factory for creating store backends from a database URL.

Uses SQLAlchemy 2.0 async engine with aiosqlite or asyncpg as the
underlying DBAPI driver.  Returns a unified ``Store`` that implements
all four protocols (EventStore, JobQueue, StateStore, AuthStore).
"""

from __future__ import annotations

from tanren_core.store.engine import create_engine_from_url, create_session_factory
from tanren_core.store.models import Base
from tanren_core.store.repository import Store


async def create_sqlite_store(db_path: str) -> Store:
    """Create a SQLite-backed store.

    Args:
        db_path: Filesystem path for the SQLite database.

    Returns:
        A ``Store`` instance implementing EventStore, JobQueue, StateStore, AuthStore.
    """
    engine, is_sqlite = create_engine_from_url(db_path)
    sf = create_session_factory(engine)
    async with engine.begin() as conn:
        await conn.run_sync(Base.metadata.create_all)
    return Store(sf, is_sqlite=is_sqlite, engine=engine)


async def create_postgres_store(dsn: str) -> Store:
    """Create a Postgres-backed store.

    Args:
        dsn: PostgreSQL connection string (``postgresql://...``).

    Returns:
        A ``Store`` instance implementing EventStore, JobQueue, StateStore, AuthStore.
    """
    engine, is_sqlite = create_engine_from_url(dsn)
    sf = create_session_factory(engine)
    async with engine.begin() as conn:
        await conn.run_sync(Base.metadata.create_all)
    return Store(sf, is_sqlite=is_sqlite, engine=engine)


async def create_store(db_url: str) -> Store:
    """Create a store from a database URL.

    For SQLite paths, returns a SQLite-backed store.
    For PostgreSQL URLs, returns a Postgres-backed store.

    Args:
        db_url: A filesystem path for SQLite or ``postgresql://`` URL.

    Returns:
        A ``Store`` instance implementing all store protocols.

    May raise ``ValueError`` if the URL scheme is not recognised
    (propagated from :func:`create_engine_from_url`).
    """
    engine, is_sqlite = create_engine_from_url(db_url)
    sf = create_session_factory(engine)
    async with engine.begin() as conn:
        await conn.run_sync(Base.metadata.create_all)
    return Store(sf, is_sqlite=is_sqlite, engine=engine)
