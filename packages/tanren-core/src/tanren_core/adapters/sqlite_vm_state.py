"""SQLite-backed VM state store for tracking assignments."""

from __future__ import annotations

import logging
from datetime import UTC, datetime
from pathlib import Path

import aiosqlite

from tanren_core.adapters.remote_types import VMAssignment

logger = logging.getLogger(__name__)

_SCHEMA = """\
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


class SqliteVMStateStore:
    """Persists VM assignment state to SQLite.

    Implements the VMStateStore protocol. Same lazy-connect pattern
    as SqliteEventEmitter.
    """

    def __init__(self, db_path: str | Path) -> None:
        """Initialize with the path to the SQLite VM state database."""
        self._db_path = Path(db_path)
        self._conn: aiosqlite.Connection | None = None

    async def _ensure_conn(self) -> aiosqlite.Connection:
        if self._conn is None:
            self._db_path.parent.mkdir(parents=True, exist_ok=True)
            self._conn = await aiosqlite.connect(str(self._db_path))
            await self._conn.executescript(_SCHEMA)
        return self._conn

    async def record_assignment(
        self,
        vm_id: str,
        workflow_id: str,
        project: str,
        spec: str,
        host: str,
    ) -> None:
        """Record a new VM assignment."""
        conn = await self._ensure_conn()
        now = datetime.now(UTC).isoformat()
        await conn.execute(
            "INSERT OR REPLACE INTO vm_assignments "
            "(vm_id, workflow_id, project, spec, host, assigned_at) "
            "VALUES (?, ?, ?, ?, ?, ?)",
            (vm_id, workflow_id, project, spec, host, now),
        )
        await conn.commit()

    async def record_release(self, vm_id: str) -> None:
        """Mark a VM assignment as released."""
        conn = await self._ensure_conn()
        now = datetime.now(UTC).isoformat()
        await conn.execute(
            "UPDATE vm_assignments SET released_at = ? WHERE vm_id = ? AND released_at IS NULL",
            (now, vm_id),
        )
        await conn.commit()

    async def get_active_assignments(self) -> list[VMAssignment]:
        """Get all currently active (unreleased) VM assignments.

        Returns:
            List of active VMAssignment records.
        """
        conn = await self._ensure_conn()
        cursor = await conn.execute(
            "SELECT vm_id, workflow_id, project, spec, host, assigned_at "
            "FROM vm_assignments WHERE released_at IS NULL"
        )
        rows = await cursor.fetchall()
        return [
            VMAssignment(
                vm_id=row[0],
                workflow_id=row[1],
                project=row[2],
                spec=row[3],
                host=row[4],
                assigned_at=row[5],
            )
            for row in rows
        ]

    async def get_assignment(self, vm_id: str) -> VMAssignment | None:
        """Get a specific VM assignment by ID.

        Returns:
            VMAssignment if found, None otherwise.
        """
        conn = await self._ensure_conn()
        cursor = await conn.execute(
            "SELECT vm_id, workflow_id, project, spec, host, assigned_at "
            "FROM vm_assignments WHERE vm_id = ? AND released_at IS NULL",
            (vm_id,),
        )
        row = await cursor.fetchone()
        if row is None:
            return None
        return VMAssignment(
            vm_id=row[0],
            workflow_id=row[1],
            project=row[2],
            spec=row[3],
            host=row[4],
            assigned_at=row[5],
        )

    async def close(self) -> None:
        """Close the database connection."""
        if self._conn is not None:
            await self._conn.close()
            self._conn = None
