"""Integration tests for Postgres backend (requires --postgres-url)."""

from __future__ import annotations

import pytest

from tanren_core.adapters.events import DispatchReceived
from tanren_core.adapters.postgres_emitter import PostgresEventEmitter
from tanren_core.adapters.postgres_event_reader import PostgresEventReader
from tanren_core.adapters.postgres_pool import create_postgres_pool, ensure_schema
from tanren_core.adapters.postgres_vm_state import PostgresVMStateStore

pytestmark = pytest.mark.postgres


@pytest.fixture
def postgres_url(request):
    url = request.config.getoption("--postgres-url")
    assert url is not None, "--postgres-url not provided (run with -m postgres)"
    return url


@pytest.fixture
async def pg_pool(postgres_url):
    pool = await create_postgres_pool(postgres_url)
    yield pool
    # Clean up tables
    async with pool.acquire() as conn:
        await conn.execute("DROP TABLE IF EXISTS events")
        await conn.execute("DROP TABLE IF EXISTS vm_assignments")
    await pool.close()


class TestEmitAndQueryRoundtrip:
    async def test_emit_and_query_roundtrip(self, pg_pool):
        emitter = PostgresEventEmitter(pg_pool)
        reader = PostgresEventReader(pg_pool)

        event = DispatchReceived(
            timestamp="2026-01-01T00:00:00Z",
            workflow_id="wf-roundtrip",
            phase="do-task",
            project="test-proj",
            cli="claude",
        )
        await emitter.emit(event)

        result = await reader.query_events(workflow_id="wf-roundtrip")
        assert result.total == 1
        assert len(result.events) == 1
        assert result.events[0].workflow_id == "wf-roundtrip"
        assert result.events[0].event_type == "DispatchReceived"
        assert result.events[0].payload["type"] == "dispatch_received"


class TestVMStateLifecycle:
    async def test_vm_state_lifecycle(self, pg_pool):
        store = PostgresVMStateStore(pg_pool)

        # Record assignment
        await store.record_assignment("vm-pg-1", "wf-1", "proj", "spec", "10.0.0.1")

        # Get active
        active = await store.get_active_assignments()
        assert len(active) == 1
        assert active[0].vm_id == "vm-pg-1"

        # Get by id
        assignment = await store.get_assignment("vm-pg-1")
        assert assignment is not None
        assert assignment.host == "10.0.0.1"

        # Release
        await store.record_release("vm-pg-1")

        # Verify empty
        active = await store.get_active_assignments()
        assert len(active) == 0

        # Get by id returns None after release
        assignment = await store.get_assignment("vm-pg-1")
        assert assignment is None


class TestSchemaIdempotent:
    async def test_schema_idempotent(self, pg_pool):
        # ensure_schema is called during pool creation, calling it again should be fine
        await ensure_schema(pg_pool)
        await ensure_schema(pg_pool)

        # Tables should still be usable
        emitter = PostgresEventEmitter(pg_pool)
        event = DispatchReceived(
            timestamp="2026-01-01T00:00:00Z",
            workflow_id="wf-idempotent",
            phase="do-task",
            project="test-proj",
            cli="claude",
        )
        await emitter.emit(event)


class TestPoolSharedAcrossAdapters:
    async def test_pool_shared_across_adapters(self, pg_pool):
        emitter = PostgresEventEmitter(pg_pool)
        reader = PostgresEventReader(pg_pool)
        store = PostgresVMStateStore(pg_pool)

        # All share the same pool
        assert emitter._pool is pg_pool
        assert reader._pool is pg_pool
        assert store._pool is pg_pool

        # All work with the same pool
        event = DispatchReceived(
            timestamp="2026-01-01T00:00:00Z",
            workflow_id="wf-shared",
            phase="do-task",
            project="test-proj",
            cli="claude",
        )
        await emitter.emit(event)
        await store.record_assignment("vm-shared", "wf-shared", "proj", "spec", "10.0.0.2")

        result = await reader.query_events(workflow_id="wf-shared")
        assert result.total >= 1

        active = await store.get_active_assignments()
        assert any(a.vm_id == "vm-shared" for a in active)
