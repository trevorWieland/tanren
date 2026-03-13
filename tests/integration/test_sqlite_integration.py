"""Integration tests for SQLite-backed event emitter and VM state store."""

from __future__ import annotations

from pathlib import Path

import aiosqlite
import pytest

from tanren_core.adapters.events import DispatchReceived
from tanren_core.adapters.remote_types import VMAssignment
from tanren_core.adapters.sqlite_emitter import SqliteEventEmitter
from tanren_core.adapters.sqlite_vm_state import SqliteVMStateStore


def _make_event(workflow_id: str = "wf-1") -> DispatchReceived:
    return DispatchReceived(
        timestamp="2026-01-01T00:00:00Z",
        workflow_id=workflow_id,
        phase="plan",
        project="myproject",
        cli="tanren",
    )


# ---------------------------------------------------------------------------
# SqliteEventEmitter tests
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_emit_creates_db_and_stores_event(tmp_path: Path) -> None:
    db_path = tmp_path / "events.db"
    emitter = SqliteEventEmitter(db_path)
    try:
        event = _make_event()
        await emitter.emit(event)

        async with aiosqlite.connect(str(db_path)) as conn:
            cursor = await conn.execute("SELECT COUNT(*) FROM events")
            row = await cursor.fetchone()
            assert row is not None
            assert row[0] == 1

            cursor = await conn.execute("SELECT workflow_id, event_type FROM events")
            row = await cursor.fetchone()
            assert row is not None
            assert row[0] == "wf-1"
            assert row[1] == "DispatchReceived"
    finally:
        await emitter.close()


@pytest.mark.asyncio
async def test_emit_multiple_events(tmp_path: Path) -> None:
    db_path = tmp_path / "events.db"
    emitter = SqliteEventEmitter(db_path)
    try:
        for i in range(3):
            await emitter.emit(_make_event(workflow_id=f"wf-{i}"))

        async with aiosqlite.connect(str(db_path)) as conn:
            cursor = await conn.execute("SELECT COUNT(*) FROM events")
            row = await cursor.fetchone()
            assert row is not None
            assert row[0] == 3
    finally:
        await emitter.close()


@pytest.mark.asyncio
async def test_lazy_connection(tmp_path: Path) -> None:
    db_path = tmp_path / "events.db"
    emitter = SqliteEventEmitter(db_path)
    try:
        assert emitter._conn is None

        await emitter.emit(_make_event())

        assert emitter._conn is not None
    finally:
        await emitter.close()


@pytest.mark.asyncio
async def test_close_sets_conn_none(tmp_path: Path) -> None:
    db_path = tmp_path / "events.db"
    emitter = SqliteEventEmitter(db_path)
    await emitter.emit(_make_event())
    assert emitter._conn is not None

    await emitter.close()
    assert emitter._conn is None


@pytest.mark.asyncio
async def test_reopen_after_close(tmp_path: Path) -> None:
    db_path = tmp_path / "events.db"
    emitter = SqliteEventEmitter(db_path)
    try:
        await emitter.emit(_make_event(workflow_id="wf-first"))
        await emitter.close()

        await emitter.emit(_make_event(workflow_id="wf-second"))

        async with aiosqlite.connect(str(db_path)) as conn:
            cursor = await conn.execute("SELECT COUNT(*) FROM events")
            row = await cursor.fetchone()
            assert row is not None
            assert row[0] == 2
    finally:
        await emitter.close()


@pytest.mark.asyncio
async def test_schema_idempotent(tmp_path: Path) -> None:
    db_path = tmp_path / "events.db"

    emitter1 = SqliteEventEmitter(db_path)
    await emitter1.emit(_make_event(workflow_id="wf-a"))
    await emitter1.close()

    emitter2 = SqliteEventEmitter(db_path)
    try:
        await emitter2.emit(_make_event(workflow_id="wf-b"))

        async with aiosqlite.connect(str(db_path)) as conn:
            cursor = await conn.execute("SELECT COUNT(*) FROM events")
            row = await cursor.fetchone()
            assert row is not None
            assert row[0] == 2
    finally:
        await emitter2.close()


# ---------------------------------------------------------------------------
# SqliteVMStateStore tests
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_record_assignment_stores_row(tmp_path: Path) -> None:
    db_path = tmp_path / "vm.db"
    store = SqliteVMStateStore(db_path)
    try:
        await store.record_assignment("vm-1", "wf-1", "proj", "spec", "1.2.3.4")

        async with aiosqlite.connect(str(db_path)) as conn:
            cursor = await conn.execute(
                "SELECT vm_id, workflow_id, project, spec, host FROM vm_assignments"
            )
            row = await cursor.fetchone()
            assert row is not None
            assert row[0] == "vm-1"
            assert row[1] == "wf-1"
            assert row[2] == "proj"
            assert row[3] == "spec"
            assert row[4] == "1.2.3.4"
    finally:
        await store.close()


@pytest.mark.asyncio
async def test_get_active_assignments(tmp_path: Path) -> None:
    db_path = tmp_path / "vm.db"
    store = SqliteVMStateStore(db_path)
    try:
        await store.record_assignment("vm-1", "wf-1", "proj1", "spec1", "1.1.1.1")
        await store.record_assignment("vm-2", "wf-2", "proj2", "spec2", "2.2.2.2")

        active = await store.get_active_assignments()
        assert len(active) == 2
        assert all(isinstance(a, VMAssignment) for a in active)
        vm_ids = {a.vm_id for a in active}
        assert vm_ids == {"vm-1", "vm-2"}
    finally:
        await store.close()


@pytest.mark.asyncio
async def test_record_release_deactivates(tmp_path: Path) -> None:
    db_path = tmp_path / "vm.db"
    store = SqliteVMStateStore(db_path)
    try:
        await store.record_assignment("vm-1", "wf-1", "proj", "spec", "1.2.3.4")
        await store.record_release("vm-1")

        active = await store.get_active_assignments()
        assert len(active) == 0
    finally:
        await store.close()


@pytest.mark.asyncio
async def test_get_assignment_by_id(tmp_path: Path) -> None:
    db_path = tmp_path / "vm.db"
    store = SqliteVMStateStore(db_path)
    try:
        await store.record_assignment("vm-1", "wf-1", "proj", "spec", "1.2.3.4")

        assignment = await store.get_assignment("vm-1")
        assert assignment is not None
        assert isinstance(assignment, VMAssignment)
        assert assignment.vm_id == "vm-1"
        assert assignment.workflow_id == "wf-1"
        assert assignment.project == "proj"
        assert assignment.spec == "spec"
        assert assignment.host == "1.2.3.4"
    finally:
        await store.close()


@pytest.mark.asyncio
async def test_get_assignment_released_returns_none(tmp_path: Path) -> None:
    db_path = tmp_path / "vm.db"
    store = SqliteVMStateStore(db_path)
    try:
        await store.record_assignment("vm-1", "wf-1", "proj", "spec", "1.2.3.4")
        await store.record_release("vm-1")

        assignment = await store.get_assignment("vm-1")
        assert assignment is None
    finally:
        await store.close()


@pytest.mark.asyncio
async def test_get_assignment_nonexistent_returns_none(tmp_path: Path) -> None:
    db_path = tmp_path / "vm.db"
    store = SqliteVMStateStore(db_path)
    try:
        assignment = await store.get_assignment("nope")
        assert assignment is None
    finally:
        await store.close()


@pytest.mark.asyncio
async def test_multiple_assignments_release_one(tmp_path: Path) -> None:
    db_path = tmp_path / "vm.db"
    store = SqliteVMStateStore(db_path)
    try:
        await store.record_assignment("vm-1", "wf-1", "proj1", "spec1", "1.1.1.1")
        await store.record_assignment("vm-2", "wf-2", "proj2", "spec2", "2.2.2.2")
        await store.record_release("vm-1")

        active = await store.get_active_assignments()
        assert len(active) == 1
        assert active[0].vm_id == "vm-2"
    finally:
        await store.close()


@pytest.mark.asyncio
async def test_close_and_reopen_persists(tmp_path: Path) -> None:
    db_path = tmp_path / "vm.db"

    store1 = SqliteVMStateStore(db_path)
    await store1.record_assignment("vm-1", "wf-1", "proj", "spec", "1.2.3.4")
    await store1.close()

    store2 = SqliteVMStateStore(db_path)
    try:
        active = await store2.get_active_assignments()
        assert len(active) == 1
        assert active[0].vm_id == "vm-1"
        assert active[0].workflow_id == "wf-1"
    finally:
        await store2.close()
