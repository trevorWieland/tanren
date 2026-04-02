"""Unified VM state store using SQLAlchemy ORM.

Replaces ``SqliteVMStateStore`` and ``PostgresVMStateStore`` with a single
implementation that works with any SQLAlchemy async engine.
"""

from __future__ import annotations

from datetime import UTC, datetime
from typing import TYPE_CHECKING

from sqlalchemy import select, update

from tanren_core.adapters.remote_types import VMAssignment
from tanren_core.store.models import VMAssignment as VMAssignmentModel

if TYPE_CHECKING:
    from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker


class VMStateRepository:
    """Persists VM assignment state via SQLAlchemy ORM.

    Satisfies the VMStateStore protocol via structural typing.
    """

    def __init__(self, session_factory: async_sessionmaker[AsyncSession]) -> None:
        """Initialize with a session factory (shared with the main store)."""
        self._sf = session_factory

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
        async with self._sf.begin() as session:
            # Use merge for INSERT OR REPLACE semantics
            await session.merge(
                VMAssignmentModel(
                    vm_id=vm_id,
                    workflow_id=workflow_id,
                    project=project,
                    spec=spec,
                    host=host,
                    assigned_at=now,
                )
            )

    async def record_release(self, vm_id: str) -> None:
        """Mark a VM assignment as released."""
        now = datetime.now(UTC).isoformat()
        async with self._sf.begin() as session:
            await session.execute(
                update(VMAssignmentModel)
                .where(VMAssignmentModel.vm_id == vm_id)
                .where(VMAssignmentModel.released_at.is_(None))
                .values(released_at=now)
            )

    async def get_active_assignments(self) -> list[VMAssignment]:
        """Get all currently active (unreleased) VM assignments.

        Returns:
            List of active VMAssignment records.
        """
        async with self._sf() as session:
            rows = (
                (
                    await session.execute(
                        select(VMAssignmentModel).where(VMAssignmentModel.released_at.is_(None))
                    )
                )
                .scalars()
                .all()
            )
            return [
                VMAssignment(
                    vm_id=r.vm_id,
                    workflow_id=r.workflow_id,
                    project=r.project,
                    spec=r.spec,
                    host=r.host,
                    assigned_at=r.assigned_at,
                )
                for r in rows
            ]

    async def get_assignment(self, vm_id: str) -> VMAssignment | None:
        """Get a specific VM assignment by ID.

        Returns:
            VMAssignment if found and active, None otherwise.
        """
        async with self._sf() as session:
            row = (
                (
                    await session.execute(
                        select(VMAssignmentModel)
                        .where(VMAssignmentModel.vm_id == vm_id)
                        .where(VMAssignmentModel.released_at.is_(None))
                    )
                )
                .scalars()
                .first()
            )
            if row is None:
                return None
            return VMAssignment(
                vm_id=row.vm_id,
                workflow_id=row.workflow_id,
                project=row.project,
                spec=row.spec,
                host=row.host,
                assigned_at=row.assigned_at,
            )

    async def close(self) -> None:
        """No-op — session factory lifecycle managed by the engine owner."""
