"""Tests for SqliteVMStateStore."""

from pathlib import Path

import aiosqlite
import pytest

from tanren_core.adapters.sqlite_vm_state import SqliteVMStateStore


@pytest.fixture
async def store(tmp_path: Path):
    """Yield an initialized SqliteVMStateStore backed by a temp database."""
    db_path = tmp_path / "vm_state.db"
    s = SqliteVMStateStore(db_path)
    # Force table creation by touching the connection.
    await s._ensure_conn()
    yield s
    await s.close()


class TestSqliteVMStateStore:
    @pytest.mark.asyncio
    async def test_init_creates_table(self, tmp_path: Path):
        db_path = tmp_path / "vm_state.db"
        store = SqliteVMStateStore(db_path)
        await store._ensure_conn()
        await store.close()

        assert db_path.exists()

        # Verify the table exists by re-opening and querying it.
        async with aiosqlite.connect(str(db_path)) as conn:
            cursor = await conn.execute(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='vm_assignments'"
            )
            row = await cursor.fetchone()
            assert row is not None
            assert row[0] == "vm_assignments"

    @pytest.mark.asyncio
    async def test_record_assignment_stores(self, store: SqliteVMStateStore):
        await store.record_assignment(
            vm_id="vm-1",
            workflow_id="wf-proj-1-1000",
            project="proj",
            spec="do-task",
            host="10.0.0.1",
        )

        assignment = await store.get_assignment("vm-1")
        assert assignment is not None
        assert assignment.vm_id == "vm-1"
        assert assignment.workflow_id == "wf-proj-1-1000"
        assert assignment.project == "proj"
        assert assignment.spec == "do-task"
        assert assignment.host == "10.0.0.1"
        assert assignment.assigned_at  # non-empty timestamp

    @pytest.mark.asyncio
    async def test_get_active_assignments_returns_unreleased(self, store: SqliteVMStateStore):
        await store.record_assignment("vm-1", "wf-1", "proj", "spec-a", "10.0.0.1")
        await store.record_assignment("vm-2", "wf-2", "proj", "spec-b", "10.0.0.2")

        active = await store.get_active_assignments()
        vm_ids = {a.vm_id for a in active}
        assert vm_ids == {"vm-1", "vm-2"}

    @pytest.mark.asyncio
    async def test_get_active_assignments_empty_when_none(self, store: SqliteVMStateStore):
        active = await store.get_active_assignments()
        assert active == []

    @pytest.mark.asyncio
    async def test_record_release_marks_released(self, store: SqliteVMStateStore):
        await store.record_assignment("vm-1", "wf-1", "proj", "spec-a", "10.0.0.1")

        await store.record_release("vm-1")

        # get_assignment only returns active (unreleased) assignments.
        assert await store.get_assignment("vm-1") is None

        active = await store.get_active_assignments()
        assert active == []

    @pytest.mark.asyncio
    async def test_get_assignment_returns_specific(self, store: SqliteVMStateStore):
        await store.record_assignment("vm-1", "wf-1", "proj", "spec-a", "10.0.0.1")
        await store.record_assignment("vm-2", "wf-2", "proj", "spec-b", "10.0.0.2")

        a = await store.get_assignment("vm-2")
        assert a is not None
        assert a.vm_id == "vm-2"
        assert a.workflow_id == "wf-2"
        assert a.host == "10.0.0.2"

    @pytest.mark.asyncio
    async def test_get_assignment_returns_none_for_unknown(self, store: SqliteVMStateStore):
        result = await store.get_assignment("vm-nonexistent")
        assert result is None

    @pytest.mark.asyncio
    async def test_multiple_assignments_and_releases(self, store: SqliteVMStateStore):
        # Assign three VMs.
        await store.record_assignment("vm-1", "wf-1", "proj", "spec-a", "10.0.0.1")
        await store.record_assignment("vm-2", "wf-2", "proj", "spec-b", "10.0.0.2")
        await store.record_assignment("vm-3", "wf-3", "proj", "spec-c", "10.0.0.3")

        # Release the middle one.
        await store.record_release("vm-2")

        active = await store.get_active_assignments()
        vm_ids = {a.vm_id for a in active}
        assert vm_ids == {"vm-1", "vm-3"}

        # Released VM is no longer returned by get_assignment.
        assert await store.get_assignment("vm-2") is None

        # Release the rest.
        await store.record_release("vm-1")
        await store.record_release("vm-3")

        assert await store.get_active_assignments() == []
