"""Tests for dispatch_orchestrator — shared dispatch lifecycle operations."""

from __future__ import annotations

from unittest.mock import AsyncMock

import pytest

from tanren_core.dispatch_orchestrator import (
    ActiveExecuteTeardownError,
    ConcurrentExecuteError,
    DispatchGuardError,
    DispatchResult,
    DuplicateTeardownError,
    PostTeardownExecuteError,
    StepEnqueueResult,
    check_execute_guards,
    check_teardown_guards,
    create_dispatch,
    enqueue_dry_run_step,
    enqueue_execute_step,
    enqueue_teardown_step,
    get_provision_result,
)
from tanren_core.env.environment_schema import EnvironmentProfile, EnvironmentProfileType
from tanren_core.schemas import Cli, Dispatch, Phase
from tanren_core.store.enums import (
    DispatchMode,
    Lane,
    StepStatus,
    StepType,
)
from tanren_core.store.handle import PersistedEnvironmentHandle
from tanren_core.store.payloads import ProvisionResult
from tanren_core.store.views import StepView

# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

_PROFILE = EnvironmentProfile(name="default", type=EnvironmentProfileType.LOCAL)


def _dispatch(workflow_id: str = "wf-test-1") -> Dispatch:
    return Dispatch(
        workflow_id=workflow_id,
        project="testproj",
        phase=Phase.DO_TASK,
        branch="main",
        spec_folder=".",
        cli=Cli.CLAUDE,
        timeout=600,
        environment_profile="default",
        resolved_profile=_PROFILE,
    )


def _step(
    step_type: StepType,
    status: StepStatus,
    seq: int = 0,
    result_json: str | None = None,
) -> StepView:
    return StepView(
        step_id=f"s-{step_type}-{seq}",
        dispatch_id="wf-test-1",
        step_type=step_type,
        step_sequence=seq,
        lane=Lane.IMPL if step_type == StepType.EXECUTE else None,
        status=status,
        worker_id=None,
        result_json=result_json,
        error=None,
        retry_count=0,
        created_at="2026-01-01T00:00:00Z",
        updated_at="2026-01-01T00:00:00Z",
    )


def _handle() -> PersistedEnvironmentHandle:
    return PersistedEnvironmentHandle(
        env_id="env-1",
        worktree_path="/tmp/wt",
        branch="main",
        project="testproj",
        provision_timestamp="2026-01-01T00:00:00Z",
    )


def _stores():
    event_store = AsyncMock()
    job_queue = AsyncMock()
    state_store = AsyncMock()
    state_store.get_steps_for_dispatch = AsyncMock(return_value=[])
    return event_store, job_queue, state_store


# ---------------------------------------------------------------------------
# create_dispatch
# ---------------------------------------------------------------------------


class TestCreateDispatch:
    async def test_creates_projection_event_and_step(self) -> None:
        event_store, job_queue, state_store = _stores()
        dispatch = _dispatch()

        result = await create_dispatch(
            dispatch=dispatch,
            mode=DispatchMode.AUTO,
            event_store=event_store,
            job_queue=job_queue,
            state_store=state_store,
            user_id="u-1",
        )

        assert isinstance(result, DispatchResult)
        assert result.dispatch_id == "wf-test-1"
        assert result.dispatch is dispatch

        # Projection created
        state_store.create_dispatch_projection.assert_awaited_once()
        proj_call = state_store.create_dispatch_projection.call_args
        assert proj_call.kwargs["dispatch_id"] == "wf-test-1"
        assert proj_call.kwargs["mode"] == DispatchMode.AUTO
        assert proj_call.kwargs["user_id"] == "u-1"

        # Event appended
        event_store.append.assert_awaited_once()
        event = event_store.append.call_args[0][0]
        assert event.type == "dispatch_created"
        assert event.entity_id == "wf-test-1"

        # Step enqueued
        job_queue.enqueue_step.assert_awaited_once()
        step_call = job_queue.enqueue_step.call_args
        assert step_call.kwargs["step_type"] == "provision"
        assert step_call.kwargs["step_sequence"] == 0

    async def test_preserve_on_failure_override(self) -> None:
        event_store, job_queue, state_store = _stores()
        dispatch = _dispatch()

        await create_dispatch(
            dispatch=dispatch,
            mode=DispatchMode.MANUAL,
            event_store=event_store,
            job_queue=job_queue,
            state_store=state_store,
            preserve_on_failure=True,
        )

        proj_call = state_store.create_dispatch_projection.call_args
        assert proj_call.kwargs["preserve_on_failure"] is True

    async def test_manual_mode(self) -> None:
        event_store, job_queue, state_store = _stores()
        dispatch = _dispatch()

        result = await create_dispatch(
            dispatch=dispatch,
            mode=DispatchMode.MANUAL,
            event_store=event_store,
            job_queue=job_queue,
            state_store=state_store,
        )

        assert result.dispatch_id == "wf-test-1"
        event = event_store.append.call_args[0][0]
        assert event.mode == DispatchMode.MANUAL


# ---------------------------------------------------------------------------
# check_execute_guards
# ---------------------------------------------------------------------------


class TestExecuteGuards:
    async def test_passes_when_no_conflicting_steps(self) -> None:
        _, _, state_store = _stores()
        state_store.get_steps_for_dispatch.return_value = [
            _step(StepType.PROVISION, StepStatus.COMPLETED, seq=0),
        ]
        await check_execute_guards(state_store, "wf-test-1")

    async def test_raises_on_concurrent_execute(self) -> None:
        _, _, state_store = _stores()
        state_store.get_steps_for_dispatch.return_value = [
            _step(StepType.PROVISION, StepStatus.COMPLETED, seq=0),
            _step(StepType.EXECUTE, StepStatus.RUNNING, seq=1),
        ]
        with pytest.raises(ConcurrentExecuteError):
            await check_execute_guards(state_store, "wf-test-1")

    async def test_raises_on_pending_execute(self) -> None:
        _, _, state_store = _stores()
        state_store.get_steps_for_dispatch.return_value = [
            _step(StepType.EXECUTE, StepStatus.PENDING, seq=1),
        ]
        with pytest.raises(ConcurrentExecuteError):
            await check_execute_guards(state_store, "wf-test-1")

    async def test_raises_after_teardown(self) -> None:
        _, _, state_store = _stores()
        state_store.get_steps_for_dispatch.return_value = [
            _step(StepType.PROVISION, StepStatus.COMPLETED, seq=0),
            _step(StepType.TEARDOWN, StepStatus.COMPLETED, seq=2),
        ]
        with pytest.raises(PostTeardownExecuteError):
            await check_execute_guards(state_store, "wf-test-1")

    async def test_allows_after_failed_execute(self) -> None:
        _, _, state_store = _stores()
        state_store.get_steps_for_dispatch.return_value = [
            _step(StepType.PROVISION, StepStatus.COMPLETED, seq=0),
            _step(StepType.EXECUTE, StepStatus.FAILED, seq=1),
        ]
        # Should NOT raise — failed execute allows re-execute
        await check_execute_guards(state_store, "wf-test-1")

    async def test_guard_error_hierarchy(self) -> None:
        assert issubclass(ConcurrentExecuteError, DispatchGuardError)
        assert issubclass(PostTeardownExecuteError, DispatchGuardError)


# ---------------------------------------------------------------------------
# check_teardown_guards
# ---------------------------------------------------------------------------


class TestTeardownGuards:
    async def test_passes_when_no_conflicting_steps(self) -> None:
        _, _, state_store = _stores()
        state_store.get_steps_for_dispatch.return_value = [
            _step(StepType.PROVISION, StepStatus.COMPLETED, seq=0),
            _step(StepType.EXECUTE, StepStatus.COMPLETED, seq=1),
        ]
        await check_teardown_guards(state_store, "wf-test-1")

    async def test_raises_on_active_execute(self) -> None:
        _, _, state_store = _stores()
        state_store.get_steps_for_dispatch.return_value = [
            _step(StepType.EXECUTE, StepStatus.RUNNING, seq=1),
        ]
        with pytest.raises(ActiveExecuteTeardownError):
            await check_teardown_guards(state_store, "wf-test-1")

    async def test_raises_on_duplicate_teardown(self) -> None:
        _, _, state_store = _stores()
        state_store.get_steps_for_dispatch.return_value = [
            _step(StepType.TEARDOWN, StepStatus.COMPLETED, seq=2),
        ]
        with pytest.raises(DuplicateTeardownError):
            await check_teardown_guards(state_store, "wf-test-1")

    async def test_raises_on_pending_teardown(self) -> None:
        _, _, state_store = _stores()
        state_store.get_steps_for_dispatch.return_value = [
            _step(StepType.TEARDOWN, StepStatus.PENDING, seq=2),
        ]
        with pytest.raises(DuplicateTeardownError):
            await check_teardown_guards(state_store, "wf-test-1")

    async def test_failed_teardown_does_not_block(self) -> None:
        """A failed teardown does not block re-enqueue (matches RunService behaviour)."""
        _, _, state_store = _stores()
        state_store.get_steps_for_dispatch.return_value = [
            _step(StepType.PROVISION, StepStatus.COMPLETED, seq=0),
            _step(StepType.TEARDOWN, StepStatus.FAILED, seq=1),
        ]
        # Default: does NOT block — FAILED is not in {PENDING, RUNNING, COMPLETED}
        await check_teardown_guards(state_store, "wf-test-1")

        # With allow_retry_after_failure: also passes (identical for FAILED-only)
        await check_teardown_guards(state_store, "wf-test-1", allow_retry_after_failure=True)

    async def test_completed_teardown_blocked_even_with_retry_flag(self) -> None:
        """allow_retry_after_failure still blocks non-failed teardowns."""
        _, _, state_store = _stores()
        state_store.get_steps_for_dispatch.return_value = [
            _step(StepType.TEARDOWN, StepStatus.COMPLETED, seq=1),
        ]
        with pytest.raises(DuplicateTeardownError):
            await check_teardown_guards(state_store, "wf-test-1", allow_retry_after_failure=True)

    async def test_guard_error_hierarchy(self) -> None:
        assert issubclass(ActiveExecuteTeardownError, DispatchGuardError)
        assert issubclass(DuplicateTeardownError, DispatchGuardError)


# ---------------------------------------------------------------------------
# enqueue_execute_step
# ---------------------------------------------------------------------------


class TestEnqueueExecuteStep:
    async def test_enqueues_with_correct_sequence(self) -> None:
        _, job_queue, state_store = _stores()
        state_store.get_steps_for_dispatch.return_value = [
            _step(StepType.PROVISION, StepStatus.COMPLETED, seq=0),
        ]

        result = await enqueue_execute_step(
            dispatch_id="wf-test-1",
            exec_dispatch=_dispatch(),
            handle=_handle(),
            job_queue=job_queue,
            state_store=state_store,
        )

        assert isinstance(result, StepEnqueueResult)
        assert result.dispatch_id == "wf-test-1"
        assert result.step_sequence == 1

        step_call = job_queue.enqueue_step.call_args
        assert step_call.kwargs["step_type"] == "execute"
        assert step_call.kwargs["step_sequence"] == 1
        assert step_call.kwargs["lane"] == str(Lane.IMPL)

    async def test_increments_sequence_for_multi_phase(self) -> None:
        _, job_queue, state_store = _stores()
        state_store.get_steps_for_dispatch.return_value = [
            _step(StepType.PROVISION, StepStatus.COMPLETED, seq=0),
            _step(StepType.EXECUTE, StepStatus.COMPLETED, seq=1),
        ]

        result = await enqueue_execute_step(
            dispatch_id="wf-test-1",
            exec_dispatch=_dispatch(),
            handle=_handle(),
            job_queue=job_queue,
            state_store=state_store,
        )

        assert result.step_sequence == 2

    async def test_checks_guards_by_default(self) -> None:
        _, job_queue, state_store = _stores()
        state_store.get_steps_for_dispatch.return_value = [
            _step(StepType.EXECUTE, StepStatus.RUNNING, seq=1),
        ]

        with pytest.raises(ConcurrentExecuteError):
            await enqueue_execute_step(
                dispatch_id="wf-test-1",
                exec_dispatch=_dispatch(),
                handle=_handle(),
                job_queue=job_queue,
                state_store=state_store,
            )

    async def test_skips_guards_when_disabled(self) -> None:
        _, job_queue, state_store = _stores()
        state_store.get_steps_for_dispatch.return_value = [
            _step(StepType.EXECUTE, StepStatus.RUNNING, seq=1),
        ]

        # Should not raise even though guard would normally block
        result = await enqueue_execute_step(
            dispatch_id="wf-test-1",
            exec_dispatch=_dispatch(),
            handle=_handle(),
            job_queue=job_queue,
            state_store=state_store,
            check_guards=False,
        )

        assert result.step_sequence == 2


# ---------------------------------------------------------------------------
# enqueue_teardown_step
# ---------------------------------------------------------------------------


class TestEnqueueTeardownStep:
    async def test_enqueues_teardown(self) -> None:
        _, job_queue, state_store = _stores()
        state_store.get_steps_for_dispatch.return_value = [
            _step(StepType.PROVISION, StepStatus.COMPLETED, seq=0),
            _step(StepType.EXECUTE, StepStatus.COMPLETED, seq=1),
        ]

        result = await enqueue_teardown_step(
            dispatch_id="wf-test-1",
            dispatch=_dispatch(),
            handle=_handle(),
            job_queue=job_queue,
            state_store=state_store,
        )

        assert result.step_sequence == 2
        step_call = job_queue.enqueue_step.call_args
        assert step_call.kwargs["step_type"] == "teardown"
        assert step_call.kwargs["lane"] is None

    async def test_checks_guards_by_default(self) -> None:
        _, job_queue, state_store = _stores()
        state_store.get_steps_for_dispatch.return_value = [
            _step(StepType.EXECUTE, StepStatus.RUNNING, seq=1),
        ]

        with pytest.raises(ActiveExecuteTeardownError):
            await enqueue_teardown_step(
                dispatch_id="wf-test-1",
                dispatch=_dispatch(),
                handle=_handle(),
                job_queue=job_queue,
                state_store=state_store,
            )

    async def test_skips_guards_when_disabled(self) -> None:
        _, job_queue, state_store = _stores()
        state_store.get_steps_for_dispatch.return_value = [
            _step(StepType.TEARDOWN, StepStatus.COMPLETED, seq=1),
        ]

        result = await enqueue_teardown_step(
            dispatch_id="wf-test-1",
            dispatch=_dispatch(),
            handle=_handle(),
            job_queue=job_queue,
            state_store=state_store,
            check_guards=False,
        )

        assert result.step_sequence == 2

    async def test_preserve_flag_passed_through(self) -> None:
        _, job_queue, state_store = _stores()
        state_store.get_steps_for_dispatch.return_value = [
            _step(StepType.PROVISION, StepStatus.COMPLETED, seq=0),
        ]

        await enqueue_teardown_step(
            dispatch_id="wf-test-1",
            dispatch=_dispatch(),
            handle=_handle(),
            job_queue=job_queue,
            state_store=state_store,
            preserve=True,
        )

        payload_json = job_queue.enqueue_step.call_args.kwargs["payload_json"]
        from tanren_core.store.payloads import TeardownStepPayload

        payload = TeardownStepPayload.model_validate_json(payload_json)
        assert payload.preserve is True

    async def test_allow_retry_after_failure(self) -> None:
        _, job_queue, state_store = _stores()
        state_store.get_steps_for_dispatch.return_value = [
            _step(StepType.PROVISION, StepStatus.COMPLETED, seq=0),
            _step(StepType.TEARDOWN, StepStatus.FAILED, seq=1),
        ]

        result = await enqueue_teardown_step(
            dispatch_id="wf-test-1",
            dispatch=_dispatch(),
            handle=_handle(),
            job_queue=job_queue,
            state_store=state_store,
            allow_retry_after_failure=True,
        )

        assert result.step_sequence == 2


# ---------------------------------------------------------------------------
# enqueue_dry_run_step
# ---------------------------------------------------------------------------


class TestEnqueueDryRunStep:
    async def test_creates_dispatch_and_enqueues_dry_run(self) -> None:
        event_store, job_queue, state_store = _stores()
        dispatch = _dispatch("vm-dryrun-test-abc")

        result = await enqueue_dry_run_step(
            dispatch=dispatch,
            mode=DispatchMode.MANUAL,
            event_store=event_store,
            job_queue=job_queue,
            state_store=state_store,
            user_id="u-1",
        )

        assert result.dispatch_id == "vm-dryrun-test-abc"

        # Projection
        proj_call = state_store.create_dispatch_projection.call_args
        assert proj_call.kwargs["preserve_on_failure"] is False

        # Event
        event = event_store.append.call_args[0][0]
        assert event.type == "dispatch_created"

        # Step type is dry_run
        step_call = job_queue.enqueue_step.call_args
        assert step_call.kwargs["step_type"] == "dry_run"


# ---------------------------------------------------------------------------
# get_provision_result
# ---------------------------------------------------------------------------


class TestGetProvisionResult:
    async def test_returns_result_from_completed_step(self) -> None:
        _, _, state_store = _stores()
        handle = _handle()
        result_json = ProvisionResult(handle=handle).model_dump_json()
        state_store.get_steps_for_dispatch.return_value = [
            _step(StepType.PROVISION, StepStatus.COMPLETED, seq=0, result_json=result_json),
        ]

        result = await get_provision_result(state_store, "wf-test-1")
        assert isinstance(result, ProvisionResult)
        assert result.handle.env_id == "env-1"

    async def test_raises_when_no_provision(self) -> None:
        _, _, state_store = _stores()
        state_store.get_steps_for_dispatch.return_value = []

        with pytest.raises(ValueError, match="No completed provision"):
            await get_provision_result(state_store, "wf-test-1")

    async def test_raises_when_provision_not_completed(self) -> None:
        _, _, state_store = _stores()
        state_store.get_steps_for_dispatch.return_value = [
            _step(StepType.PROVISION, StepStatus.RUNNING, seq=0),
        ]

        with pytest.raises(ValueError, match="No completed provision"):
            await get_provision_result(state_store, "wf-test-1")
