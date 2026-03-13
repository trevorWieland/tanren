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
