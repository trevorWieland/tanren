"""Async engine and session factory for SQLAlchemy 2.0.

Handles dialect routing (SQLite vs Postgres), connection pragmas,
and pool configuration.  All store access goes through the
``async_sessionmaker`` produced by :func:`create_session_factory`.
"""

from __future__ import annotations

from pathlib import Path

from sqlalchemy import event
from sqlalchemy.ext.asyncio import (
    AsyncEngine,
    AsyncSession,
    async_sessionmaker,
    create_async_engine,
)
from sqlalchemy.pool import StaticPool

from tanren_core.adapters.postgres_pool import is_postgres_url


def create_engine_from_url(db_url: str) -> tuple[AsyncEngine, bool]:
    """Create an async engine from a database URL.

    Args:
        db_url: A filesystem path (SQLite) or ``postgresql://`` DSN.

    Returns:
        Tuple of (engine, is_sqlite).

    Raises:
        ValueError: If the URL scheme is not recognised.
    """
    if is_postgres_url(db_url):
        sa_url = db_url
        if sa_url.startswith("postgresql://"):
            sa_url = sa_url.replace("postgresql://", "postgresql+asyncpg://", 1)
        elif sa_url.startswith("postgres://"):
            sa_url = sa_url.replace("postgres://", "postgresql+asyncpg://", 1)
        engine = create_async_engine(sa_url, pool_size=2, max_overflow=8)
        return engine, False

    if "://" in db_url and not db_url.startswith("sqlite"):
        msg = (
            f"Unsupported database URL scheme: {db_url.split('://', maxsplit=1)[0]}://. "
            "Use a filesystem path for SQLite or postgresql:// for Postgres."
        )
        raise ValueError(msg)

    # SQLite — single connection, WAL mode, foreign keys
    sa_url = db_url if db_url.startswith("sqlite") else f"sqlite+aiosqlite:///{db_url}"

    # Ensure parent directories exist for filesystem paths (matches old SqliteStore behaviour)
    if not sa_url.endswith(":memory:") and ":///" in sa_url:
        db_file = sa_url.split(":///", maxsplit=1)[1]
        if db_file:
            Path(db_file).parent.mkdir(parents=True, exist_ok=True)

    engine = create_async_engine(
        sa_url,
        # StaticPool keeps a single connection alive for the engine's lifetime.
        # Required for in-memory databases and matches the current single-conn pattern.
        # StaticPool does not accept pool_size/max_overflow.
        poolclass=StaticPool,
    )

    @event.listens_for(engine.sync_engine, "connect")
    def _set_sqlite_pragma(dbapi_conn, _connection_record) -> None:  # noqa: ANN001 — DBAPI types
        cursor = dbapi_conn.cursor()
        cursor.execute("PRAGMA journal_mode=WAL")
        cursor.execute("PRAGMA foreign_keys=ON")
        cursor.close()

    return engine, True


def create_session_factory(engine: AsyncEngine) -> async_sessionmaker[AsyncSession]:
    """Create an async session factory bound to the given engine.

    Returns:
        Configured ``async_sessionmaker`` with ``expire_on_commit=False``.
    """
    return async_sessionmaker(engine, expire_on_commit=False)
