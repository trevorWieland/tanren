"""Tests for VMStateRepository — unified VM assignment store."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

from tanren_core.adapters.vm_state_repository import VMStateRepository
from tanren_core.store.engine import create_engine_from_url, create_session_factory
from tanren_core.store.models import Base

if TYPE_CHECKING:
    from pathlib import Path


@pytest.fixture
async def store(tmp_path: Path):
    """Yield an initialized VMStateRepository backed by a temp database."""
    db_path = str(tmp_path / "vm-state.db")
    engine, _is_sqlite = create_engine_from_url(db_path)
    sf = create_session_factory(engine)
    async with engine.begin() as conn:
        await conn.run_sync(Base.metadata.create_all)
    yield VMStateRepository(sf)
    await engine.dispose()


class TestVMStateRepository:
    async def test_lazy_creates_db(self, tmp_path: Path) -> None:
        db_path = str(tmp_path / "sub" / "vm.db")
        engine, _is_sqlite = create_engine_from_url(db_path)
        sf = create_session_factory(engine)
        async with engine.begin() as conn:
            await conn.run_sync(Base.metadata.create_all)
        repo = VMStateRepository(sf)
        assignments = await repo.get_active_assignments()
        assert assignments == []
        await engine.dispose()

    async def test_record_assignment_stores(self, store: VMStateRepository) -> None:
        await store.record_assignment(
            vm_id="vm-1",
            workflow_id="wf-1",
            project="proj",
            spec="spec/001",
            host="10.0.0.1",
        )
        assignments = await store.get_active_assignments()
        assert len(assignments) == 1
        a = assignments[0]
        assert a.vm_id == "vm-1"
        assert a.workflow_id == "wf-1"
        assert a.project == "proj"
        assert a.spec == "spec/001"
        assert a.host == "10.0.0.1"
        assert a.assigned_at  # non-empty

    async def test_get_active_assignments_returns_unreleased(
        self, store: VMStateRepository
    ) -> None:
        await store.record_assignment("vm-1", "wf-1", "p", "s", "h1")
        await store.record_assignment("vm-2", "wf-2", "p", "s", "h2")
        await store.record_release("vm-1")
        active = await store.get_active_assignments()
        assert len(active) == 1
        assert active[0].vm_id == "vm-2"

    async def test_get_active_assignments_empty_when_none(self, store: VMStateRepository) -> None:
        active = await store.get_active_assignments()
        assert active == []

    async def test_record_release_marks_released(self, store: VMStateRepository) -> None:
        await store.record_assignment("vm-1", "wf-1", "p", "s", "h1")
        await store.record_release("vm-1")
        active = await store.get_active_assignments()
        assert active == []

    async def test_get_assignment_returns_specific(self, store: VMStateRepository) -> None:
        await store.record_assignment("vm-1", "wf-1", "p", "s", "h1")
        a = await store.get_assignment("vm-1")
        assert a is not None
        assert a.vm_id == "vm-1"

    async def test_get_assignment_returns_none_for_unknown(self, store: VMStateRepository) -> None:
        result = await store.get_assignment("nonexistent")
        assert result is None

    async def test_multiple_assignments_and_releases(self, store: VMStateRepository) -> None:
        for i in range(3):
            await store.record_assignment(f"vm-{i}", f"wf-{i}", "p", "s", f"h{i}")

        await store.record_release("vm-0")
        await store.record_release("vm-2")

        active = await store.get_active_assignments()
        assert len(active) == 1
        assert active[0].vm_id == "vm-1"
