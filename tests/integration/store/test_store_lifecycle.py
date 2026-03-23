"""Integration test for the full dispatch lifecycle through the store."""

from __future__ import annotations

import json
from typing import TYPE_CHECKING

import pytest

from tanren_core.env.environment_schema import EnvironmentProfile
from tanren_core.schemas import AuthMode, Cli, Dispatch, Outcome, Phase
from tanren_core.store.enums import DispatchMode, DispatchStatus, Lane, StepStatus, StepType
from tanren_core.store.events import DispatchCompleted, DispatchCreated
from tanren_core.store.factory import create_sqlite_store
from tanren_core.store.payloads import ProvisionStepPayload
from tanren_core.store.views import DispatchListFilter

if TYPE_CHECKING:
    from pathlib import Path

    from tanren_core.store.sqlite import SqliteStore

DEFAULT_PROFILE = EnvironmentProfile(name="default")


def _make_dispatch(workflow_id: str = "wf-test-1-100") -> Dispatch:
    return Dispatch(
        workflow_id=workflow_id,
        phase=Phase.DO_TASK,
        project="test",
        spec_folder="spec/001",
        branch="main",
        cli=Cli.CLAUDE,
        auth=AuthMode.API_KEY,
        timeout=1800,
        resolved_profile=DEFAULT_PROFILE,
    )


@pytest.fixture
async def store(tmp_path: Path):
    s = await create_sqlite_store(str(tmp_path / "lifecycle.db"))
    yield s
    await s.close()


class TestFullDispatchLifecycle:
    """End-to-end: create dispatch → enqueue provision → dequeue → ack → verify."""

    async def test_dispatch_lifecycle(self, store: SqliteStore) -> None:
        dispatch = _make_dispatch()
        dispatch_json = dispatch.model_dump_json()

        # 1. Create dispatch projection
        await store.create_dispatch_projection(
            dispatch_id="wf-test-1-100",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=dispatch_json,
        )

        # 2. Append DispatchCreated event
        event = DispatchCreated(
            timestamp="2026-01-01T00:00:00Z",
            workflow_id="wf-test-1-100",
            dispatch=dispatch,
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
        )
        await store.append(event)

        # 3. Enqueue provision step
        payload = ProvisionStepPayload(dispatch=dispatch)
        await store.enqueue_step(
            step_id="step-prov",
            dispatch_id="wf-test-1-100",
            step_type="provision",
            step_sequence=0,
            lane=None,
            payload_json=payload.model_dump_json(),
        )

        # Verify dispatch is now running
        view = await store.get_dispatch("wf-test-1-100")
        assert view is not None
        assert view.status == DispatchStatus.RUNNING

        # 4. Dequeue provision step
        step = await store.dequeue(lane=None, worker_id="w1", max_concurrent=10)
        assert step is not None
        assert step.step_id == "step-prov"
        assert step.step_type == StepType.PROVISION

        # 5. Ack with result
        result = json.dumps({"handle": {"env_id": "env-001"}})
        await store.ack(step.step_id, result_json=result)

        # 6. Verify step completed
        step_view = await store.get_step("step-prov")
        assert step_view is not None
        assert step_view.status == StepStatus.COMPLETED
        assert step_view.result_json is not None

        # 7. Enqueue execute step
        await store.enqueue_step(
            step_id="step-exec",
            dispatch_id="wf-test-1-100",
            step_type="execute",
            step_sequence=1,
            lane="impl",
            payload_json="{}",
        )

        # 8. Dequeue and ack execute
        exec_step = await store.dequeue(lane=Lane.IMPL, worker_id="w1", max_concurrent=1)
        assert exec_step is not None
        assert exec_step.step_type == StepType.EXECUTE
        await store.ack(exec_step.step_id, result_json='{"outcome": "success"}')

        # 9. Enqueue teardown step
        await store.enqueue_step(
            step_id="step-td",
            dispatch_id="wf-test-1-100",
            step_type="teardown",
            step_sequence=2,
            lane=None,
            payload_json="{}",
        )

        # 10. Dequeue and ack teardown
        td_step = await store.dequeue(lane=None, worker_id="w1", max_concurrent=10)
        assert td_step is not None
        assert td_step.step_type == StepType.TEARDOWN
        await store.ack(td_step.step_id, result_json='{"vm_released": true}')

        # 11. Mark dispatch completed
        await store.append(
            DispatchCompleted(
                timestamp="2026-01-01T00:01:00Z",
                workflow_id="wf-test-1-100",
                outcome=Outcome.SUCCESS,
                total_duration_secs=60,
            )
        )
        await store.update_dispatch_status(
            "wf-test-1-100", DispatchStatus.COMPLETED, Outcome.SUCCESS
        )

        # 12. Verify final state
        final = await store.get_dispatch("wf-test-1-100")
        assert final is not None
        assert final.status == DispatchStatus.COMPLETED
        assert final.outcome == Outcome.SUCCESS

        steps = await store.get_steps_for_dispatch("wf-test-1-100")
        assert len(steps) == 3
        assert all(s.status == StepStatus.COMPLETED for s in steps)

        # 13. Verify event trail
        events = await store.query_events(dispatch_id="wf-test-1-100")
        assert events.total >= 5  # DispatchCreated + 3 StepEnqueued + DispatchCompleted

    async def test_concurrent_lane_isolation(self, store: SqliteStore) -> None:
        """Steps on different lanes don't interfere."""
        for i, (_cli, lane) in enumerate([
            (Cli.CLAUDE, "impl"),
            (Cli.CODEX, "audit"),
            (Cli.BASH, "gate"),
        ]):
            d = _make_dispatch(workflow_id=f"wf-test-{i}-100")
            await store.create_dispatch_projection(
                dispatch_id=f"wf-test-{i}-100",
                mode=DispatchMode.AUTO,
                lane=Lane(lane),
                preserve_on_failure=False,
                dispatch_json=d.model_dump_json(),
            )
            await store.enqueue_step(
                step_id=f"step-{i}",
                dispatch_id=f"wf-test-{i}-100",
                step_type="execute",
                step_sequence=1,
                lane=lane,
                payload_json="{}",
            )

        # Each lane should have exactly one step
        impl = await store.dequeue(lane=Lane.IMPL, worker_id="w1", max_concurrent=1)
        assert impl is not None
        assert impl.dispatch_id == "wf-test-0-100"

        audit = await store.dequeue(lane=Lane.AUDIT, worker_id="w1", max_concurrent=1)
        assert audit is not None
        assert audit.dispatch_id == "wf-test-1-100"

        gate = await store.dequeue(lane=Lane.GATE, worker_id="w1", max_concurrent=3)
        assert gate is not None
        assert gate.dispatch_id == "wf-test-2-100"

    async def test_query_dispatches_by_project(self, store: SqliteStore) -> None:
        """Query dispatches filtering by project."""
        for i in range(3):
            d = _make_dispatch(workflow_id=f"wf-test-{i}-100")
            await store.create_dispatch_projection(
                dispatch_id=f"wf-test-{i}-100",
                mode=DispatchMode.AUTO,
                lane=Lane.IMPL,
                preserve_on_failure=False,
                dispatch_json=d.model_dump_json(),
            )

        results = await store.query_dispatches(DispatchListFilter(project="test", limit=10))
        assert len(results) == 3

        results = await store.query_dispatches(DispatchListFilter(project="nonexistent", limit=10))
        assert len(results) == 0

    async def test_cancel_pending_steps(self, store: SqliteStore) -> None:
        """cancel_pending_steps stops pending steps from being dequeued."""
        dispatch = _make_dispatch(workflow_id="wf-cancel-100")
        await store.create_dispatch_projection(
            dispatch_id="wf-cancel-100",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=dispatch.model_dump_json(),
        )

        payload = ProvisionStepPayload(dispatch=dispatch)
        await store.enqueue_step(
            step_id="step-cancel-prov",
            dispatch_id="wf-cancel-100",
            step_type="provision",
            step_sequence=0,
            lane=None,
            payload_json=payload.model_dump_json(),
        )

        # Cancel all pending steps
        cancelled = await store.cancel_pending_steps("wf-cancel-100")
        assert cancelled == 1

        # Dequeue should return nothing
        step = await store.dequeue(lane=None, worker_id="w1", max_concurrent=10)
        assert step is None

        # Verify step is cancelled in the projection
        steps = await store.get_steps_for_dispatch("wf-cancel-100")
        assert len(steps) == 1
        assert steps[0].status == StepStatus.CANCELLED

    async def test_cancel_pending_steps_no_pending(self, store: SqliteStore) -> None:
        """cancel_pending_steps returns 0 when no pending steps exist."""
        dispatch = _make_dispatch(workflow_id="wf-nopending-100")
        await store.create_dispatch_projection(
            dispatch_id="wf-nopending-100",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=dispatch.model_dump_json(),
        )

        # No steps enqueued — cancel should return 0
        cancelled = await store.cancel_pending_steps("wf-nopending-100")
        assert cancelled == 0

    async def test_cancel_does_not_affect_running_steps(self, store: SqliteStore) -> None:
        """cancel_pending_steps leaves running steps untouched."""
        dispatch = _make_dispatch(workflow_id="wf-running-100")
        await store.create_dispatch_projection(
            dispatch_id="wf-running-100",
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
            preserve_on_failure=False,
            dispatch_json=dispatch.model_dump_json(),
        )

        payload = ProvisionStepPayload(dispatch=dispatch)
        await store.enqueue_step(
            step_id="step-running-prov",
            dispatch_id="wf-running-100",
            step_type="provision",
            step_sequence=0,
            lane=None,
            payload_json=payload.model_dump_json(),
        )

        # Dequeue to make it running
        step = await store.dequeue(lane=None, worker_id="w1", max_concurrent=10)
        assert step is not None

        # Cancel should not affect the running step
        cancelled = await store.cancel_pending_steps("wf-running-100")
        assert cancelled == 0

        # Step should still be running
        steps = await store.get_steps_for_dispatch("wf-running-100")
        assert steps[0].status == StepStatus.RUNNING
