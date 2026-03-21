"""Tests for lifecycle event serialization and deserialization."""

import pytest

from tanren_core.env.environment_schema import EnvironmentProfile
from tanren_core.schemas import AuthMode, Cli, Dispatch, Outcome, Phase
from tanren_core.store.enums import DispatchMode, Lane, StepType
from tanren_core.store.events import (
    DispatchCompleted,
    DispatchCreated,
    DispatchFailed,
    StepCompleted,
    StepDequeued,
    StepEnqueued,
    StepFailed,
    StepStarted,
)
from tanren_core.store.handle import PersistedEnvironmentHandle
from tanren_core.store.payloads import ExecuteResult, ProvisionResult, TeardownResult

DEFAULT_PROFILE = EnvironmentProfile(name="default")


def _make_dispatch() -> Dispatch:
    return Dispatch(
        workflow_id="wf-test-1-100",
        phase=Phase.DO_TASK,
        project="test",
        spec_folder="spec/001",
        branch="main",
        cli=Cli.CLAUDE,
        auth=AuthMode.API_KEY,
        timeout=1800,
        resolved_profile=DEFAULT_PROFILE,
    )


def _make_handle() -> PersistedEnvironmentHandle:
    return PersistedEnvironmentHandle(
        env_id="env-abc",
        worktree_path="/workspace/test",
        branch="main",
        project="test",
        provision_timestamp="2026-01-01T00:00:00Z",
    )


class TestDispatchCreated:
    def test_roundtrip(self) -> None:
        event = DispatchCreated(
            timestamp="2026-01-01T00:00:00Z",
            workflow_id="wf-test-1-100",
            dispatch=_make_dispatch(),
            mode=DispatchMode.AUTO,
            lane=Lane.IMPL,
        )
        restored = DispatchCreated.model_validate_json(event.model_dump_json())
        assert restored.dispatch.project == "test"
        assert restored.mode == DispatchMode.AUTO
        assert restored.lane == Lane.IMPL
        assert restored.type == "dispatch_created"


class TestDispatchCompleted:
    def test_roundtrip(self) -> None:
        event = DispatchCompleted(
            timestamp="2026-01-01T00:01:00Z",
            workflow_id="wf-test-1-100",
            outcome=Outcome.SUCCESS,
            total_duration_secs=60,
        )
        restored = DispatchCompleted.model_validate_json(event.model_dump_json())
        assert restored.outcome == Outcome.SUCCESS
        assert restored.total_duration_secs == 60


class TestDispatchFailed:
    def test_roundtrip(self) -> None:
        event = DispatchFailed(
            timestamp="2026-01-01T00:01:00Z",
            workflow_id="wf-test-1-100",
            failed_step_id="step-123",
            failed_step_type=StepType.EXECUTE,
            error="timeout exceeded",
        )
        restored = DispatchFailed.model_validate_json(event.model_dump_json())
        assert restored.failed_step_type == StepType.EXECUTE
        assert restored.outcome == Outcome.ERROR  # default


class TestStepEnqueued:
    def test_with_lane(self) -> None:
        event = StepEnqueued(
            timestamp="2026-01-01T00:00:00Z",
            workflow_id="wf-test-1-100",
            step_id="step-001",
            step_type=StepType.EXECUTE,
            step_sequence=1,
            lane=Lane.IMPL,
        )
        assert event.lane == Lane.IMPL

    def test_without_lane(self) -> None:
        event = StepEnqueued(
            timestamp="2026-01-01T00:00:00Z",
            workflow_id="wf-test-1-100",
            step_id="step-000",
            step_type=StepType.PROVISION,
            step_sequence=0,
        )
        assert event.lane is None

    def test_roundtrip(self) -> None:
        event = StepEnqueued(
            timestamp="2026-01-01T00:00:00Z",
            workflow_id="wf-test-1-100",
            step_id="step-002",
            step_type=StepType.TEARDOWN,
            step_sequence=2,
        )
        restored = StepEnqueued.model_validate_json(event.model_dump_json())
        assert restored == event


class TestStepDequeued:
    def test_roundtrip(self) -> None:
        event = StepDequeued(
            timestamp="2026-01-01T00:00:00Z",
            workflow_id="wf-test-1-100",
            step_id="step-001",
            worker_id="worker-alpha",
        )
        restored = StepDequeued.model_validate_json(event.model_dump_json())
        assert restored.worker_id == "worker-alpha"


class TestStepStarted:
    def test_roundtrip(self) -> None:
        event = StepStarted(
            timestamp="2026-01-01T00:00:00Z",
            workflow_id="wf-test-1-100",
            step_id="step-001",
            worker_id="worker-alpha",
            step_type=StepType.PROVISION,
        )
        restored = StepStarted.model_validate_json(event.model_dump_json())
        assert restored.step_type == StepType.PROVISION


class TestStepCompleted:
    def test_provision_result_roundtrip(self) -> None:
        event = StepCompleted(
            timestamp="2026-01-01T00:00:00Z",
            workflow_id="wf-test-1-100",
            step_id="step-000",
            step_type=StepType.PROVISION,
            duration_secs=30,
            result_payload=ProvisionResult(handle=_make_handle()),
        )
        json_str = event.model_dump_json()
        restored = StepCompleted.model_validate_json(json_str)
        assert isinstance(restored.result_payload, ProvisionResult)
        assert restored.result_payload.handle.env_id == "env-abc"

    def test_execute_result_roundtrip(self) -> None:
        event = StepCompleted(
            timestamp="2026-01-01T00:00:00Z",
            workflow_id="wf-test-1-100",
            step_id="step-001",
            step_type=StepType.EXECUTE,
            duration_secs=120,
            result_payload=ExecuteResult(
                outcome=Outcome.SUCCESS,
                exit_code=0,
                duration_secs=120,
            ),
        )
        json_str = event.model_dump_json()
        restored = StepCompleted.model_validate_json(json_str)
        assert isinstance(restored.result_payload, ExecuteResult)
        assert restored.result_payload.outcome == Outcome.SUCCESS

    def test_teardown_result_roundtrip(self) -> None:
        event = StepCompleted(
            timestamp="2026-01-01T00:00:00Z",
            workflow_id="wf-test-1-100",
            step_id="step-002",
            step_type=StepType.TEARDOWN,
            duration_secs=5,
            result_payload=TeardownResult(estimated_cost=0.10),
        )
        json_str = event.model_dump_json()
        restored = StepCompleted.model_validate_json(json_str)
        assert isinstance(restored.result_payload, TeardownResult)
        assert restored.result_payload.estimated_cost == pytest.approx(0.10)


class TestStepFailed:
    def test_roundtrip(self) -> None:
        event = StepFailed(
            timestamp="2026-01-01T00:00:00Z",
            workflow_id="wf-test-1-100",
            step_id="step-001",
            step_type=StepType.EXECUTE,
            error="SSH connection refused",
            error_class="transient",
            retry_count=2,
            duration_secs=5,
        )
        restored = StepFailed.model_validate_json(event.model_dump_json())
        assert restored.error_class == "transient"
        assert restored.retry_count == 2

    def test_defaults(self) -> None:
        event = StepFailed(
            timestamp="2026-01-01T00:00:00Z",
            workflow_id="wf-test-1-100",
            step_id="step-001",
            step_type=StepType.PROVISION,
            error="out of VMs",
            duration_secs=0,
        )
        assert event.error_class is None
        assert event.retry_count == 0
