"""Postgres-backed VM state store for tracking assignments."""

from __future__ import annotations

import logging
from datetime import UTC, datetime
from typing import TYPE_CHECKING

from tanren_core.adapters.remote_types import VMAssignment

if TYPE_CHECKING:
    import asyncpg

logger = logging.getLogger(__name__)


class PostgresVMStateStore:
    """Persists VM assignment state to Postgres via a shared pool.

    Satisfies the VMStateStore protocol via structural typing.
    The pool is owned externally; close() is a no-op.
    """

    def __init__(self, pool: asyncpg.Pool) -> None:
        """Initialize with an existing asyncpg pool."""
        self._pool = pool

    async def record_assignment(
        self,
        vm_id: str,
        workflow_id: str,
        project: str,
        spec: str,
        host: str,
    ) -> None:
        """Record a new VM assignment."""
        now = datetime.now(UTC).isoformat()
        await self._pool.execute(
            "INSERT INTO vm_assignments "
            "(vm_id, workflow_id, project, spec, host, assigned_at) "
            "VALUES ($1, $2, $3, $4, $5, $6) "
            "ON CONFLICT (vm_id) DO UPDATE SET "
            "workflow_id = EXCLUDED.workflow_id, "
            "project = EXCLUDED.project, "
            "spec = EXCLUDED.spec, "
            "host = EXCLUDED.host, "
            "assigned_at = EXCLUDED.assigned_at, "
            "released_at = NULL",
            vm_id,
            workflow_id,
            project,
            spec,
            host,
            now,
        )

    async def record_release(self, vm_id: str) -> None:
        """Mark a VM assignment as released."""
        now = datetime.now(UTC).isoformat()
        await self._pool.execute(
            "UPDATE vm_assignments SET released_at = $1 WHERE vm_id = $2 AND released_at IS NULL",
            now,
            vm_id,
        )

    async def get_active_assignments(self) -> list[VMAssignment]:
        """Get all currently active (unreleased) VM assignments.

        Returns:
            List of active VMAssignment records.
        """
        rows = await self._pool.fetch(
            "SELECT vm_id, workflow_id, project, spec, host, assigned_at "
            "FROM vm_assignments WHERE released_at IS NULL"
        )
        return [
            VMAssignment(
                vm_id=row["vm_id"],
                workflow_id=row["workflow_id"],
                project=row["project"],
                spec=row["spec"],
                host=row["host"],
                assigned_at=row["assigned_at"],
            )
            for row in rows
        ]

    async def get_assignment(self, vm_id: str) -> VMAssignment | None:
        """Get a specific VM assignment by ID.

        Returns:
            VMAssignment if found, None otherwise.
        """
        row = await self._pool.fetchrow(
            "SELECT vm_id, workflow_id, project, spec, host, assigned_at "
            "FROM vm_assignments WHERE vm_id = $1 AND released_at IS NULL",
            vm_id,
        )
        if row is None:
            return None
        return VMAssignment(
            vm_id=row["vm_id"],
            workflow_id=row["workflow_id"],
            project=row["project"],
            spec=row["spec"],
            host=row["host"],
            assigned_at=row["assigned_at"],
        )

    async def close(self) -> None:
        """No-op — pool is owned externally."""
