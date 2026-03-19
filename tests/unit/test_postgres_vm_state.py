"""Tests for the Postgres VM state store."""

from __future__ import annotations

from unittest.mock import AsyncMock, MagicMock

from tanren_core.adapters.postgres_vm_state import PostgresVMStateStore
from tanren_core.adapters.protocols import VMStateStore
from tanren_core.adapters.remote_types import VMAssignment


def _mock_pool():
    pool = MagicMock()
    pool.execute = AsyncMock()
    pool.fetch = AsyncMock(return_value=[])
    pool.fetchrow = AsyncMock(return_value=None)
    return pool


class TestRecordAssignment:
    async def test_record_assignment(self):
        pool = _mock_pool()
        store = PostgresVMStateStore(pool)

        await store.record_assignment("vm-1", "wf-1", "proj", "spec", "1.2.3.4")

        pool.execute.assert_awaited_once()
        sql = pool.execute.call_args[0][0]
        assert "INSERT INTO vm_assignments" in sql
        assert "ON CONFLICT" in sql


class TestRecordRelease:
    async def test_record_release(self):
        pool = _mock_pool()
        store = PostgresVMStateStore(pool)

        await store.record_release("vm-1")

        pool.execute.assert_awaited_once()
        sql = pool.execute.call_args[0][0]
        assert "UPDATE vm_assignments" in sql
        assert "released_at" in sql


class TestGetActiveAssignments:
    async def test_get_active_assignments_empty(self):
        pool = _mock_pool()
        store = PostgresVMStateStore(pool)

        result = await store.get_active_assignments()

        assert result == []
        pool.fetch.assert_awaited_once()

    async def test_get_active_assignments_with_rows(self):
        pool = _mock_pool()
        row = {
            "vm_id": "vm-1",
            "workflow_id": "wf-1",
            "project": "proj",
            "spec": "spec",
            "host": "1.2.3.4",
            "assigned_at": "2026-01-01T00:00:00Z",
        }
        pool.fetch = AsyncMock(return_value=[row])
        store = PostgresVMStateStore(pool)

        result = await store.get_active_assignments()

        assert len(result) == 1
        assert isinstance(result[0], VMAssignment)
        assert result[0].vm_id == "vm-1"
        assert result[0].host == "1.2.3.4"


class TestGetAssignment:
    async def test_get_assignment_not_found(self):
        pool = _mock_pool()
        store = PostgresVMStateStore(pool)

        result = await store.get_assignment("vm-99")

        assert result is None

    async def test_get_assignment_found(self):
        pool = _mock_pool()
        row = {
            "vm_id": "vm-1",
            "workflow_id": "wf-1",
            "project": "proj",
            "spec": "spec",
            "host": "1.2.3.4",
            "assigned_at": "2026-01-01T00:00:00Z",
        }
        pool.fetchrow = AsyncMock(return_value=row)
        store = PostgresVMStateStore(pool)

        result = await store.get_assignment("vm-1")

        assert result is not None
        assert result.vm_id == "vm-1"


class TestCloseAndProtocol:
    async def test_close_is_noop(self):
        pool = _mock_pool()
        pool.close = AsyncMock()
        store = PostgresVMStateStore(pool)

        await store.close()

        pool.close.assert_not_awaited()

    def test_protocol_conformance(self):
        pool = _mock_pool()
        store = PostgresVMStateStore(pool)
        assert isinstance(store, VMStateStore)
