"""Tests for PostgresStore — EventStore, JobQueue, and StateStore protocols.

These tests require a real PostgreSQL database and are gated by the
``@pytest.mark.postgres`` marker.  Pass ``--postgres-url`` to pytest to run.
"""

from __future__ import annotations

import json

import asyncpg
import pytest

from tanren_core.env.environment_schema import EnvironmentProfile
from tanren_core.schemas import AuthMode, Cli, Dispatch, Outcome, Phase
from tanren_core.store.enums import DispatchMode, DispatchStatus, Lane, StepStatus, StepType
from tanren_core.store.events import DispatchCreated
from tanren_core.store.postgres import PostgresStore
from tanren_core.store.views import DispatchListFilter

pytestmark = pytest.mark.postgres

DEFAULT_PROFILE = EnvironmentProfile(name="default")


def _make_dispatch(
    workflow_id: str = "wf-test-1-100",
    phase: Phase = Phase.DO_TASK,
    cli: Cli = Cli.CLAUDE,
) -> Dispatch:
    return Dispatch(
        workflow_id=workflow_id,
        phase=phase,
        project="test",
        spec_folder="spec/001",
        branch="main",
        cli=cli,
        auth=AuthMode.API_KEY,
        timeout=1800,
        resolved_profile=DEFAULT_PROFILE,
    )


@pytest.fixture
async def store(request):
    pg_url = request.config.getoption("--postgres-url")
    if not pg_url:
        raise pytest.skip.Exception("--postgres-url not provided")

    pool = await asyncpg.create_pool(pg_url, min_size=1, max_size=5)
    s = PostgresStore(pool)
    await s.ensure_schema()

    # Clean tables before each test
    async with pool.acquire() as conn:
        await conn.execute("DELETE FROM step_projection")
        await conn.execute("DELETE FROM dispatch_projection")
        await conn.execute("DELETE FROM events")

    yield s

    # Clean up after test
    async with pool.acquire() as conn:
        await conn.execute("DELETE FROM step_projection")
        await conn.execute("DELETE FROM dispatch_projection")
        await conn.execute("DELETE FROM events")

    await pool.close()


# ── EventStore tests ──────────────────────────────────────────────────────


class TestEventStoreAppend:
    async def test_append_and_query(self, store: PostgresStore) -> None:
        dispatch = _make_dispatch()
        event = DispatchCreated(
            timestamp="2026-01-01T00:00:00Z",
            workflow_id="wf-test-1-100",
            dispatch=dispatch,
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
        )
        await store.append(event)

        result = await store.query_events(dispatch_id="wf-test-1-100")
        assert result.total == 1
        assert len(result.events) == 1
        assert result.events[0].event_type == "DispatchCreated"
        assert result.events[0].workflow_id == "wf-test-1-100"

    async def test_query_by_event_type(self, store: PostgresStore) -> None:
        event = DispatchCreated(
            timestamp="2026-01-01T00:00:00Z",
            workflow_id="wf-test-1-100",
            dispatch=_make_dispatch(),
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
        )
        await store.append(event)

        result = await store.query_events(event_type="DispatchCreated")
        assert result.total == 1

        result = await store.query_events(event_type="StepEnqueued")
        assert result.total == 0

    async def test_query_with_time_range(self, store: PostgresStore) -> None:
        for i in range(3):
            event = DispatchCreated(
                timestamp=f"2026-01-0{i + 1}T00:00:00Z",
                workflow_id=f"wf-test-{i}-100",
                dispatch=_make_dispatch(workflow_id=f"wf-test-{i}-100"),
                mode=DispatchMode.AUTO,
                lane=Lane.IMPL,
            )
            await store.append(event)

        result = await store.query_events(since="2026-01-02T00:00:00Z")
        assert result.total == 2

    async def test_query_pagination(self, store: PostgresStore) -> None:
        for i in range(5):
            event = DispatchCreated(
                timestamp=f"2026-01-01T00:0{i}:00Z",
                workflow_id=f"wf-test-{i}-100",
                dispatch=_make_dispatch(workflow_id=f"wf-test-{i}-100"),
                mode=DispatchMode.AUTO,
                lane=Lane.IMPL,
            )
            await store.append(event)

        result = await store.query_events(limit=2, offset=0)
        assert len(result.events) == 2
        assert result.total == 5

        result = await store.query_events(limit=2, offset=3)
        assert len(result.events) == 2

    async def test_query_empty(self, store: PostgresStore) -> None:
        result = await store.query_events(dispatch_id="nonexistent")
        assert result.total == 0
        assert result.events == []


# ── JobQueue tests ────────────────────────────────────────────────────────


class TestJobQueueEnqueueDequeue:
    async def test_enqueue_and_dequeue(self, store: PostgresStore) -> None:
        dispatch = _make_dispatch()
        await store.create_dispatch_projection(
            dispatch_id="wf-test-1-100",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=dispatch.model_dump_json(),
        )

        await store.enqueue_step(
            step_id="step-001",
            dispatch_id="wf-test-1-100",
            step_type="provision",
            step_sequence=0,
            lane=None,
            payload_json='{"test": true}',
        )

        step = await store.dequeue(lane=None, worker_id="w1", max_concurrent=10)
        assert step is not None
        assert step.step_id == "step-001"
        assert step.step_type == StepType.PROVISION
        assert step.dispatch_id == "wf-test-1-100"

    async def test_dequeue_empty(self, store: PostgresStore) -> None:
        step = await store.dequeue(lane=Lane.IMPL, worker_id="w1", max_concurrent=1)
        assert step is None

    async def test_dequeue_respects_lane(self, store: PostgresStore) -> None:
        dispatch = _make_dispatch()
        await store.create_dispatch_projection(
            dispatch_id="wf-test-1-100",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=dispatch.model_dump_json(),
        )

        await store.enqueue_step(
            step_id="step-exec",
            dispatch_id="wf-test-1-100",
            step_type="execute",
            step_sequence=1,
            lane="impl",
            payload_json="{}",
        )

        # Different lane should find nothing
        step = await store.dequeue(lane=Lane.AUDIT, worker_id="w1", max_concurrent=1)
        assert step is None

        # Correct lane should find it
        step = await store.dequeue(lane=Lane.IMPL, worker_id="w1", max_concurrent=1)
        assert step is not None
        assert step.step_id == "step-exec"


class TestJobQueueConcurrency:
    async def test_dequeue_respects_max_concurrent(self, store: PostgresStore) -> None:
        dispatch = _make_dispatch()
        await store.create_dispatch_projection(
            dispatch_id="wf-test-1-100",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=dispatch.model_dump_json(),
        )

        # Enqueue 2 steps
        for i in range(2):
            await store.enqueue_step(
                step_id=f"step-{i}",
                dispatch_id="wf-test-1-100",
                step_type="execute",
                step_sequence=i,
                lane="impl",
                payload_json="{}",
            )

        # max_concurrent=1: dequeue first
        step1 = await store.dequeue(lane=Lane.IMPL, worker_id="w1", max_concurrent=1)
        assert step1 is not None

        # max_concurrent=1: second should be blocked
        step2 = await store.dequeue(lane=Lane.IMPL, worker_id="w1", max_concurrent=1)
        assert step2 is None

        # Ack first, then second should be available
        await store.ack(step1.step_id, result_json='{"ok": true}')
        step2 = await store.dequeue(lane=Lane.IMPL, worker_id="w1", max_concurrent=1)
        assert step2 is not None


class TestJobQueueAckNack:
    async def test_ack_stores_result(self, store: PostgresStore) -> None:
        dispatch = _make_dispatch()
        await store.create_dispatch_projection(
            dispatch_id="wf-test-1-100",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=dispatch.model_dump_json(),
        )
        await store.enqueue_step(
            step_id="step-001",
            dispatch_id="wf-test-1-100",
            step_type="provision",
            step_sequence=0,
            lane=None,
            payload_json="{}",
        )

        step = await store.dequeue(lane=None, worker_id="w1", max_concurrent=10)
        assert step is not None
        await store.ack(step.step_id, result_json='{"handle": "data"}')

        view = await store.get_step(step.step_id)
        assert view is not None
        assert view.status == StepStatus.COMPLETED
        assert view.result_json is not None
        assert json.loads(view.result_json) == {"handle": "data"}

    async def test_nack_marks_failed(self, store: PostgresStore) -> None:
        dispatch = _make_dispatch()
        await store.create_dispatch_projection(
            dispatch_id="wf-test-1-100",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=dispatch.model_dump_json(),
        )
        await store.enqueue_step(
            step_id="step-001",
            dispatch_id="wf-test-1-100",
            step_type="provision",
            step_sequence=0,
            lane=None,
            payload_json="{}",
        )

        step = await store.dequeue(lane=None, worker_id="w1", max_concurrent=10)
        assert step is not None
        await store.nack(step.step_id, error="VM unavailable")

        view = await store.get_step(step.step_id)
        assert view is not None
        assert view.status == StepStatus.FAILED
        assert view.error == "VM unavailable"

    async def test_nack_with_retry(self, store: PostgresStore) -> None:
        dispatch = _make_dispatch()
        await store.create_dispatch_projection(
            dispatch_id="wf-test-1-100",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=dispatch.model_dump_json(),
        )
        await store.enqueue_step(
            step_id="step-001",
            dispatch_id="wf-test-1-100",
            step_type="execute",
            step_sequence=1,
            lane="impl",
            payload_json="{}",
        )

        step = await store.dequeue(lane=Lane.IMPL, worker_id="w1", max_concurrent=1)
        assert step is not None
        await store.nack(step.step_id, error="transient error", retry=True)

        # Should be re-dequeueable
        view = await store.get_step(step.step_id)
        assert view is not None
        assert view.status == StepStatus.PENDING
        assert view.retry_count == 1
        assert view.worker_id is None

        step2 = await store.dequeue(lane=Lane.IMPL, worker_id="w2", max_concurrent=1)
        assert step2 is not None
        assert step2.step_id == step.step_id


# ── StateStore tests ──────────────────────────────────────────────────────


class TestStateStoreDispatches:
    async def test_create_and_get_dispatch(self, store: PostgresStore) -> None:
        dispatch = _make_dispatch()
        await store.create_dispatch_projection(
            dispatch_id="wf-test-1-100",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=dispatch.model_dump_json(),
        )

        view = await store.get_dispatch("wf-test-1-100")
        assert view is not None
        assert view.dispatch_id == "wf-test-1-100"
        assert view.mode == DispatchMode.AUTO
        assert view.status == DispatchStatus.PENDING
        assert view.lane == Lane.IMPL
        assert view.dispatch.project == "test"

    async def test_get_nonexistent_dispatch(self, store: PostgresStore) -> None:
        view = await store.get_dispatch("nonexistent")
        assert view is None

    async def test_update_dispatch_status(self, store: PostgresStore) -> None:
        dispatch = _make_dispatch()
        await store.create_dispatch_projection(
            dispatch_id="wf-test-1-100",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=dispatch.model_dump_json(),
        )

        await store.update_dispatch_status(
            "wf-test-1-100", DispatchStatus.COMPLETED, Outcome.SUCCESS
        )

        view = await store.get_dispatch("wf-test-1-100")
        assert view is not None
        assert view.status == DispatchStatus.COMPLETED
        assert view.outcome == Outcome.SUCCESS

    async def test_query_dispatches_by_status(self, store: PostgresStore) -> None:
        for i in range(3):
            d = _make_dispatch(workflow_id=f"wf-test-{i}-100")
            await store.create_dispatch_projection(
                dispatch_id=f"wf-test-{i}-100",
                mode=DispatchMode.AUTO,
                lane=Lane.IMPL,
                preserve_on_failure=False,
                dispatch_json=d.model_dump_json(),
            )

        await store.update_dispatch_status(
            "wf-test-0-100", DispatchStatus.COMPLETED, Outcome.SUCCESS
        )

        results = await store.query_dispatches(DispatchListFilter(status=DispatchStatus.PENDING))
        assert len(results) == 2

        results = await store.query_dispatches(DispatchListFilter(status=DispatchStatus.COMPLETED))
        assert len(results) == 1


class TestStateStoreSteps:
    async def test_get_steps_for_dispatch(self, store: PostgresStore) -> None:
        dispatch = _make_dispatch()
        await store.create_dispatch_projection(
            dispatch_id="wf-test-1-100",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=dispatch.model_dump_json(),
        )

        for i, (st, lane) in enumerate([
            ("provision", None),
            ("execute", "impl"),
            ("teardown", None),
        ]):
            await store.enqueue_step(
                step_id=f"step-{i}",
                dispatch_id="wf-test-1-100",
                step_type=st,
                step_sequence=i,
                lane=lane,
                payload_json="{}",
            )

        steps = await store.get_steps_for_dispatch("wf-test-1-100")
        assert len(steps) == 3
        assert steps[0].step_type == StepType.PROVISION
        assert steps[1].step_type == StepType.EXECUTE
        assert steps[2].step_type == StepType.TEARDOWN

    async def test_count_running_steps(self, store: PostgresStore) -> None:
        dispatch = _make_dispatch()
        await store.create_dispatch_projection(
            dispatch_id="wf-test-1-100",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=dispatch.model_dump_json(),
        )

        await store.enqueue_step(
            step_id="step-001",
            dispatch_id="wf-test-1-100",
            step_type="execute",
            step_sequence=1,
            lane="impl",
            payload_json="{}",
        )

        assert await store.count_running_steps(lane=Lane.IMPL) == 0

        await store.dequeue(lane=Lane.IMPL, worker_id="w1", max_concurrent=1)
        assert await store.count_running_steps(lane=Lane.IMPL) == 1


# ── Lifecycle ─────────────────────────────────────────────────────────────


class TestStoreLifecycle:
    async def test_close_is_noop(self, store: PostgresStore) -> None:
        await store.close()
        # Double close should be safe
        await store.close()

    async def test_full_dispatch_lifecycle(self, store: PostgresStore) -> None:
        """End-to-end: create dispatch -> enqueue provision -> dequeue -> ack."""
        dispatch = _make_dispatch()
        dispatch_json = dispatch.model_dump_json()

        # Create dispatch
        await store.create_dispatch_projection(
            dispatch_id="wf-test-1-100",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=dispatch_json,
        )

        # Append DispatchCreated event
        event = DispatchCreated(
            timestamp="2026-01-01T00:00:00Z",
            workflow_id="wf-test-1-100",
            dispatch=dispatch,
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
        )
        await store.append(event)

        # Enqueue provision step
        payload = json.dumps({"dispatch": json.loads(dispatch_json)})
        await store.enqueue_step(
            step_id="step-prov",
            dispatch_id="wf-test-1-100",
            step_type="provision",
            step_sequence=0,
            lane=None,
            payload_json=payload,
        )

        # Dequeue and process
        step = await store.dequeue(lane=None, worker_id="w1", max_concurrent=10)
        assert step is not None
        assert step.step_id == "step-prov"

        # Ack with result
        await store.ack(step.step_id, result_json='{"handle": {"env_id": "e1"}}')

        # Verify state
        step_view = await store.get_step("step-prov")
        assert step_view is not None
        assert step_view.status == StepStatus.COMPLETED

        dispatch_view = await store.get_dispatch("wf-test-1-100")
        assert dispatch_view is not None
        assert dispatch_view.status == DispatchStatus.RUNNING  # enqueue_step set it

        # Verify events
        events = await store.query_events(dispatch_id="wf-test-1-100")
        assert events.total >= 2  # DispatchCreated + StepEnqueued
