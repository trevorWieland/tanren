"""Tests for the API state store."""

from __future__ import annotations

import asyncio
import contextlib
from datetime import UTC, datetime, timedelta

import pytest

from tanren_api.models import DispatchRunStatus, RunEnvironmentStatus
from tanren_api.state import APIStateStore, DispatchRecord, EnvironmentRecord
from tanren_core.adapters.types import EnvironmentHandle, LocalEnvironmentRuntime
from tanren_core.schemas import Cli, Dispatch, Phase


def _make_dispatch(workflow_id: str = "wf-1") -> Dispatch:
    return Dispatch(
        workflow_id=workflow_id,
        project="test",
        phase=Phase.DO_TASK,
        branch="main",
        spec_folder="specs/test",
        cli=Cli.CLAUDE,
        timeout=1800,
    )


def _make_dispatch_record(dispatch_id: str = "wf-1") -> DispatchRecord:
    return DispatchRecord(
        dispatch_id=dispatch_id,
        dispatch=_make_dispatch(dispatch_id),
        status=DispatchRunStatus.PENDING,
        created_at="2026-01-01T00:00:00Z",
    )


def _make_env_handle(env_id: str = "env-1") -> EnvironmentHandle:
    from pathlib import Path  # noqa: PLC0415

    return EnvironmentHandle(
        env_id=env_id,
        worktree_path=Path("/tmp/wt"),
        branch="main",
        project="test",
        runtime=LocalEnvironmentRuntime(),
    )


def _make_env_record(env_id: str = "env-1") -> EnvironmentRecord:
    return EnvironmentRecord(
        env_id=env_id,
        handle=_make_env_handle(env_id),
        status=RunEnvironmentStatus.PROVISIONED,
        vm_id="vm-1",
        host="10.0.0.1",
    )


@pytest.mark.api
class TestAPIStateStore:
    async def test_add_and_get_dispatch(self):
        store = APIStateStore()
        record = _make_dispatch_record()
        await store.add_dispatch(record)

        found = await store.get_dispatch("wf-1")
        assert found is not None
        assert found.dispatch_id == "wf-1"
        assert found.status == DispatchRunStatus.PENDING

    async def test_get_dispatch_not_found(self):
        store = APIStateStore()
        found = await store.get_dispatch("nonexistent")
        assert found is None

    async def test_update_dispatch(self):
        store = APIStateStore()
        await store.add_dispatch(_make_dispatch_record())

        await store.update_dispatch(
            "wf-1",
            status=DispatchRunStatus.RUNNING,
            started_at="2026-01-01T00:01:00Z",
        )

        found = await store.get_dispatch("wf-1")
        assert found is not None
        assert found.status == DispatchRunStatus.RUNNING
        assert found.started_at == "2026-01-01T00:01:00Z"

    async def test_remove_dispatch(self):
        store = APIStateStore()
        await store.add_dispatch(_make_dispatch_record())

        removed = await store.remove_dispatch("wf-1")
        assert removed is not None
        assert removed.dispatch_id == "wf-1"

        found = await store.get_dispatch("wf-1")
        assert found is None

    async def test_add_and_get_environment(self):
        store = APIStateStore()
        record = _make_env_record()
        await store.add_environment(record)

        found = await store.get_environment("env-1")
        assert found is not None
        assert found.env_id == "env-1"

    async def test_get_environment_not_found(self):
        store = APIStateStore()
        found = await store.get_environment("nonexistent")
        assert found is None

    async def test_update_environment(self):
        store = APIStateStore()
        await store.add_environment(_make_env_record())

        await store.update_environment(
            "env-1",
            status=RunEnvironmentStatus.EXECUTING,
            phase=Phase.DO_TASK,
        )

        found = await store.get_environment("env-1")
        assert found is not None
        assert found.status == RunEnvironmentStatus.EXECUTING
        assert found.phase == Phase.DO_TASK

    async def test_remove_environment(self):
        store = APIStateStore()
        await store.add_environment(_make_env_record())

        removed = await store.remove_environment("env-1")
        assert removed is not None

        found = await store.get_environment("env-1")
        assert found is None

    async def test_update_dispatch_with_task(self):
        store = APIStateStore()
        await store.add_dispatch(_make_dispatch_record())

        async def noop():
            await asyncio.sleep(3600)

        task = asyncio.create_task(noop())
        await store.update_dispatch("wf-1", task=task)

        found = await store.get_dispatch("wf-1")
        assert found is not None
        assert found.task is task
        task.cancel()
        with contextlib.suppress(asyncio.CancelledError):
            await task

    async def test_shutdown_cancels_tasks(self):
        store = APIStateStore()

        async def long_running():
            await asyncio.sleep(3600)

        record = _make_dispatch_record()
        record.task = asyncio.create_task(long_running())
        await store.add_dispatch(record)

        await store.shutdown()
        assert record.task.cancelled() or record.task.done()

    async def test_reap_removes_old_terminal_dispatches(self):
        store = APIStateStore()
        old_time = (datetime.now(UTC) - timedelta(seconds=3660)).isoformat()

        # Old COMPLETED dispatch — should be reaped
        old_completed = _make_dispatch_record("old-completed")
        old_completed.status = DispatchRunStatus.COMPLETED
        old_completed.completed_at = old_time
        async with store._lock:
            store._dispatches["old-completed"] = old_completed

        # Old RUNNING dispatch — should NOT be reaped (not terminal)
        old_running = _make_dispatch_record("old-running")
        old_running.status = DispatchRunStatus.RUNNING
        old_running.completed_at = old_time
        async with store._lock:
            store._dispatches["old-running"] = old_running

        # Recent COMPLETED dispatch — should NOT be reaped (too recent)
        recent_completed = _make_dispatch_record("recent-completed")
        recent_completed.status = DispatchRunStatus.COMPLETED
        recent_completed.completed_at = datetime.now(UTC).isoformat()
        async with store._lock:
            store._dispatches["recent-completed"] = recent_completed

        # Trigger reap via add_dispatch
        await store.add_dispatch(_make_dispatch_record("trigger"))

        assert await store.get_dispatch("old-completed") is None
        assert await store.get_dispatch("old-running") is not None
        assert await store.get_dispatch("recent-completed") is not None
        assert await store.get_dispatch("trigger") is not None

    async def test_get_dispatch_returns_defensive_copy(self):
        store = APIStateStore()
        await store.add_dispatch(_make_dispatch_record())

        copy = await store.get_dispatch("wf-1")
        assert copy is not None
        copy.status = DispatchRunStatus.FAILED

        original = await store.get_dispatch("wf-1")
        assert original is not None
        assert original.status == DispatchRunStatus.PENDING

    async def test_get_environment_returns_defensive_copy(self):
        store = APIStateStore()
        await store.add_environment(_make_env_record())

        copy = await store.get_environment("env-1")
        assert copy is not None
        copy.status = RunEnvironmentStatus.FAILED

        original = await store.get_environment("env-1")
        assert original is not None
        assert original.status == RunEnvironmentStatus.PROVISIONED

    async def test_try_transition_environment_succeeds(self):
        store = APIStateStore()
        await store.add_environment(_make_env_record())

        result = await store.try_transition_environment(
            "env-1",
            from_statuses=frozenset({RunEnvironmentStatus.PROVISIONED}),
            to_status=RunEnvironmentStatus.EXECUTING,
            phase=Phase.DO_TASK,
        )
        assert result is not None
        assert result.status == RunEnvironmentStatus.EXECUTING
        assert result.phase == Phase.DO_TASK

        # Verify the internal record was updated too
        fetched = await store.get_environment("env-1")
        assert fetched is not None
        assert fetched.status == RunEnvironmentStatus.EXECUTING

    async def test_try_transition_environment_rejects_wrong_status(self):
        store = APIStateStore()
        record = _make_env_record()
        record.status = RunEnvironmentStatus.EXECUTING
        await store.add_environment(record)

        result = await store.try_transition_environment(
            "env-1",
            from_statuses=frozenset({RunEnvironmentStatus.PROVISIONED}),
            to_status=RunEnvironmentStatus.EXECUTING,
        )
        assert result is None

    async def test_try_transition_environment_not_found(self):
        store = APIStateStore()

        result = await store.try_transition_environment(
            "nonexistent",
            from_statuses=frozenset({RunEnvironmentStatus.PROVISIONED}),
            to_status=RunEnvironmentStatus.EXECUTING,
        )
        assert result is None

    async def test_cancel_environment_task_cancels_running_task(self):
        store = APIStateStore()
        record = _make_env_record()

        async def long_running():
            await asyncio.sleep(3600)

        record.task = asyncio.create_task(long_running())
        await store.add_environment(record)

        result = await store.cancel_environment_task("env-1")
        assert result is True
        assert record.task.cancelled() or record.task.done()

    async def test_cancel_environment_task_returns_false_on_timeout(self):
        store = APIStateStore()
        record = _make_env_record()

        async def _resist_one_cancel():
            try:
                await asyncio.sleep(3600)
            except asyncio.CancelledError:
                asyncio.current_task().uncancel()  # type: ignore[union-attr]
                await asyncio.sleep(3600)

        record.task = asyncio.create_task(_resist_one_cancel())
        await store.add_environment(record)
        await asyncio.sleep(0)  # Let task enter its try block

        result = await store.cancel_environment_task("env-1", wait_secs=0.1)
        assert result is False
        assert not record.task.done()

        # Cleanup: cancel again (this time it propagates)
        record.task.cancel()
        with contextlib.suppress(asyncio.CancelledError):
            await record.task

    async def test_cancel_environment_task_returns_false_when_no_task(self):
        store = APIStateStore()
        await store.add_environment(_make_env_record())

        result = await store.cancel_environment_task("env-1")
        assert result is False

    async def test_try_transition_dispatch_succeeds(self):
        store = APIStateStore()
        record = _make_dispatch_record()
        record.status = DispatchRunStatus.RUNNING
        await store.add_dispatch(record)

        from tanren_core.schemas import Outcome  # noqa: PLC0415

        result = await store.try_transition_dispatch(
            "wf-1",
            from_statuses=frozenset({DispatchRunStatus.RUNNING}),
            to_status=DispatchRunStatus.COMPLETED,
            outcome=Outcome.SUCCESS,
            completed_at="2026-01-01T01:00:00Z",
        )
        assert result is not None
        assert result.status == DispatchRunStatus.COMPLETED
        assert result.outcome == Outcome.SUCCESS

        # Verify the internal record was updated too
        fetched = await store.get_dispatch("wf-1")
        assert fetched is not None
        assert fetched.status == DispatchRunStatus.COMPLETED

    async def test_try_transition_dispatch_rejects_wrong_status(self):
        store = APIStateStore()
        record = _make_dispatch_record()
        record.status = DispatchRunStatus.CANCELLED
        await store.add_dispatch(record)

        from tanren_core.schemas import Outcome  # noqa: PLC0415

        result = await store.try_transition_dispatch(
            "wf-1",
            from_statuses=frozenset({DispatchRunStatus.RUNNING}),
            to_status=DispatchRunStatus.COMPLETED,
            outcome=Outcome.SUCCESS,
        )
        assert result is None

        # Internal record unchanged
        fetched = await store.get_dispatch("wf-1")
        assert fetched is not None
        assert fetched.status == DispatchRunStatus.CANCELLED

    async def test_try_transition_dispatch_not_found(self):
        store = APIStateStore()

        from tanren_core.schemas import Outcome  # noqa: PLC0415

        result = await store.try_transition_dispatch(
            "nonexistent",
            from_statuses=frozenset({DispatchRunStatus.RUNNING}),
            to_status=DispatchRunStatus.COMPLETED,
            outcome=Outcome.SUCCESS,
        )
        assert result is None

    async def test_get_dispatch_deep_copies_dispatch_model(self):
        store = APIStateStore()
        await store.add_dispatch(_make_dispatch_record())

        copy = await store.get_dispatch("wf-1")
        assert copy is not None

        # The Dispatch model object should be a separate instance
        original = await store.get_dispatch("wf-1")
        assert original is not None
        assert copy.dispatch is not original.dispatch

    async def test_try_transition_environment_clears_fields_with_none(self):
        """Passing outcome=None and completed_at=None explicitly clears them."""
        from tanren_core.schemas import Outcome  # noqa: PLC0415

        store = APIStateStore()
        record = _make_env_record()
        record.status = RunEnvironmentStatus.COMPLETED
        record.outcome = Outcome.SUCCESS
        record.completed_at = "2026-01-01T01:00:00Z"
        await store.add_environment(record)

        result = await store.try_transition_environment(
            "env-1",
            from_statuses=frozenset({RunEnvironmentStatus.COMPLETED}),
            to_status=RunEnvironmentStatus.EXECUTING,
            outcome=None,
            completed_at=None,
        )
        assert result is not None
        assert result.outcome is None
        assert result.completed_at is None

        fetched = await store.get_environment("env-1")
        assert fetched is not None
        assert fetched.outcome is None
        assert fetched.completed_at is None

    async def test_unset_default_preserves_existing_values(self):
        """Not passing outcome preserves the existing value (_UNSET default)."""
        from tanren_core.schemas import Outcome  # noqa: PLC0415

        store = APIStateStore()
        record = _make_env_record()
        record.status = RunEnvironmentStatus.COMPLETED
        record.outcome = Outcome.SUCCESS
        record.completed_at = "2026-01-01T01:00:00Z"
        await store.add_environment(record)

        result = await store.try_transition_environment(
            "env-1",
            from_statuses=frozenset({RunEnvironmentStatus.COMPLETED}),
            to_status=RunEnvironmentStatus.EXECUTING,
        )
        assert result is not None
        assert result.outcome == Outcome.SUCCESS
        assert result.completed_at == "2026-01-01T01:00:00Z"

    async def test_reap_preserves_recent_terminal_dispatches(self):
        store = APIStateStore()
        recent_time = datetime.now(UTC).isoformat()

        record = _make_dispatch_record("recent-done")
        record.status = DispatchRunStatus.COMPLETED
        record.completed_at = recent_time
        async with store._lock:
            store._dispatches["recent-done"] = record

        # Trigger reap
        await store.add_dispatch(_make_dispatch_record("trigger2"))

        assert await store.get_dispatch("recent-done") is not None
