"""Tests for dispatch endpoints."""

import asyncio
import contextlib
import json
import re
from pathlib import Path
from unittest.mock import AsyncMock

import pytest

from tanren_api.models import DispatchRunStatus
from tanren_api.state import DispatchRecord
from tanren_core.schemas import Cli, Dispatch, Phase


@pytest.mark.api
class TestDispatch:
    async def test_create_dispatch_returns_accepted(self, client, auth_headers):
        resp = await client.post(
            "/api/v1/dispatch",
            json={
                "project": "test-project",
                "phase": "do-task",
                "branch": "main",
                "spec_folder": "specs/test",
                "cli": "claude",
            },
            headers=auth_headers,
        )
        assert resp.status_code == 200
        data = resp.json()
        assert "dispatch_id" in data
        assert data["status"] == "accepted"
        assert re.match(r"^wf-.*-\d+-\d+$", data["dispatch_id"])

    async def test_get_dispatch_returns_detail(self, client, auth_headers):
        # Create first
        create_resp = await client.post(
            "/api/v1/dispatch",
            json={
                "project": "test-project",
                "phase": "do-task",
                "branch": "main",
                "spec_folder": "specs/test",
                "cli": "claude",
            },
            headers=auth_headers,
        )
        dispatch_id = create_resp.json()["dispatch_id"]

        # Query
        resp = await client.get(f"/api/v1/dispatch/{dispatch_id}", headers=auth_headers)
        assert resp.status_code == 200
        data = resp.json()
        assert data["workflow_id"] == dispatch_id
        assert data["project"] == "test-project"
        assert data["phase"] == "do-task"

    async def test_get_dispatch_not_found(self, client, auth_headers):
        resp = await client.get("/api/v1/dispatch/nonexistent-id", headers=auth_headers)
        assert resp.status_code == 404

    async def test_cancel_dispatch_pending(self, client, auth_headers, app):
        # Create a dispatch without execution env so it stays PENDING
        app.state.execution_env = None

        resp = await client.post(
            "/api/v1/dispatch",
            json={
                "project": "test-project",
                "phase": "do-task",
                "branch": "main",
                "spec_folder": "specs/test",
                "cli": "claude",
            },
            headers=auth_headers,
        )
        dispatch_id = resp.json()["dispatch_id"]

        cancel_resp = await client.delete(f"/api/v1/dispatch/{dispatch_id}", headers=auth_headers)
        assert cancel_resp.status_code == 200
        data = cancel_resp.json()
        assert data["dispatch_id"] == dispatch_id
        assert data["status"] == "cancelled"

    async def test_cancel_dispatch_completed_returns_409(self, client, auth_headers, app):
        app.state.execution_env = None

        resp = await client.post(
            "/api/v1/dispatch",
            json={
                "project": "test-project",
                "phase": "do-task",
                "branch": "main",
                "spec_folder": "specs/test",
                "cli": "claude",
            },
            headers=auth_headers,
        )
        dispatch_id = resp.json()["dispatch_id"]

        # Cancel it first
        await client.delete(f"/api/v1/dispatch/{dispatch_id}", headers=auth_headers)

        # Try to cancel again — should be 409
        resp2 = await client.delete(f"/api/v1/dispatch/{dispatch_id}", headers=auth_headers)
        assert resp2.status_code == 409

    async def test_cancel_dispatch_removes_ipc_file(self, client, auth_headers, app, tmp_path):
        app.state.execution_env = None

        resp = await client.post(
            "/api/v1/dispatch",
            json={
                "project": "test-project",
                "phase": "do-task",
                "branch": "main",
                "spec_folder": "specs/test",
                "cli": "claude",
            },
            headers=auth_headers,
        )
        dispatch_id = resp.json()["dispatch_id"]

        # Verify the IPC file exists
        store = app.state.api_store
        record = await store.get_dispatch(dispatch_id)
        assert record is not None
        assert record.dispatch_path is not None
        assert record.dispatch_path.exists()

        # Cancel
        await client.delete(f"/api/v1/dispatch/{dispatch_id}", headers=auth_headers)

        # IPC file should be gone
        assert not record.dispatch_path.exists()

    async def test_create_dispatch_skips_ipc_when_execution_env_present(
        self, client, auth_headers, app
    ):
        """When execution_env is available, no IPC file is written."""
        resp = await client.post(
            "/api/v1/dispatch",
            json={
                "project": "test-project",
                "phase": "do-task",
                "branch": "main",
                "spec_folder": "specs/test",
                "cli": "claude",
            },
            headers=auth_headers,
        )
        dispatch_id = resp.json()["dispatch_id"]
        store = app.state.api_store
        record = await store.get_dispatch(dispatch_id)
        assert record is not None
        assert record.dispatch_path is None

    async def test_create_dispatch_writes_ipc_when_no_execution_env(
        self, client, auth_headers, app
    ):
        """When execution_env is None, IPC file is written for daemon pickup."""
        app.state.execution_env = None
        resp = await client.post(
            "/api/v1/dispatch",
            json={
                "project": "test-project",
                "phase": "do-task",
                "branch": "main",
                "spec_folder": "specs/test",
                "cli": "claude",
            },
            headers=auth_headers,
        )
        dispatch_id = resp.json()["dispatch_id"]
        store = app.state.api_store
        record = await store.get_dispatch(dispatch_id)
        assert record is not None
        assert record.dispatch_path is not None
        assert record.dispatch_path.exists()

    async def test_workflow_ids_unique_within_same_second(self, client, auth_headers):
        """Three rapid dispatches produce unique workflow IDs."""
        ids = set()
        for _ in range(3):
            resp = await client.post(
                "/api/v1/dispatch",
                json={
                    "project": "test-project",
                    "phase": "do-task",
                    "branch": "main",
                    "spec_folder": "specs/test",
                    "cli": "claude",
                    "issue": 1,
                },
                headers=auth_headers,
            )
            ids.add(resp.json()["dispatch_id"])
        assert len(ids) == 3

    async def test_synthetic_issue_does_not_wrap(self, client, auth_headers):
        """Synthetic issue ID (when issue=0) is a full nanosecond timestamp, not modulo-wrapped."""
        resp = await client.post(
            "/api/v1/dispatch",
            json={
                "project": "test-project",
                "phase": "do-task",
                "branch": "main",
                "spec_folder": "specs/test",
                "cli": "claude",
                "issue": 0,
            },
            headers=auth_headers,
        )
        assert resp.status_code == 200
        dispatch_id = resp.json()["dispatch_id"]
        # Extract the issue segment: wf-{project}-{issue}-{epoch}
        parts = dispatch_id.split("-")
        # project may contain hyphens, so issue is second-to-last numeric segment
        # Format: wf-test-project-{issue}-{epoch}
        # The last segment is epoch (digits), second-to-last is issue (digits)
        issue_str = parts[-2]
        issue_val = int(issue_str)
        assert issue_val > 10**8, f"Expected full nanosecond timestamp, got {issue_val}"

    async def test_cancel_dispatch_not_found(self, client, auth_headers):
        resp = await client.delete("/api/v1/dispatch/nonexistent-id", headers=auth_headers)
        assert resp.status_code == 404

    async def test_get_dispatch_resolves_daemon_result(self, client, auth_headers, app):
        """Daemon-delegated dispatch resolves when result file appears in IPC results dir."""
        app.state.execution_env = None

        resp = await client.post(
            "/api/v1/dispatch",
            json={
                "project": "test-project",
                "phase": "do-task",
                "branch": "main",
                "spec_folder": "specs/test",
                "cli": "claude",
            },
            headers=auth_headers,
        )
        dispatch_id = resp.json()["dispatch_id"]

        # Write a matching result JSON to {ipc_dir}/results/
        results_dir = Path(app.state.config.ipc_dir) / "results"
        results_dir.mkdir(parents=True, exist_ok=True)
        result_data = {
            "workflow_id": dispatch_id,
            "phase": "do-task",
            "outcome": "success",
            "signal": "complete",
            "exit_code": 0,
            "duration_secs": 42,
            "spec_modified": False,
        }
        result_file = results_dir / f"{dispatch_id}.json"
        result_file.write_text(json.dumps(result_data))

        # GET dispatch — should resolve to completed
        resp = await client.get(f"/api/v1/dispatch/{dispatch_id}", headers=auth_headers)
        assert resp.status_code == 200
        data = resp.json()
        assert data["status"] == "completed"
        assert data["outcome"] == "success"

    async def test_get_dispatch_pending_no_result_stays_pending(self, client, auth_headers, app):
        """Daemon-delegated dispatch with no result file stays pending."""
        app.state.execution_env = None

        resp = await client.post(
            "/api/v1/dispatch",
            json={
                "project": "test-project",
                "phase": "do-task",
                "branch": "main",
                "spec_folder": "specs/test",
                "cli": "claude",
            },
            headers=auth_headers,
        )
        dispatch_id = resp.json()["dispatch_id"]

        # GET dispatch — no result file, should stay pending
        resp = await client.get(f"/api/v1/dispatch/{dispatch_id}", headers=auth_headers)
        assert resp.status_code == 200
        data = resp.json()
        assert data["status"] == "pending"

    async def test_daemon_result_error_outcome_maps_to_failed(self, client, auth_headers, app):
        """Daemon result with outcome=error maps dispatch status to failed."""
        app.state.execution_env = None

        resp = await client.post(
            "/api/v1/dispatch",
            json={
                "project": "test-project",
                "phase": "do-task",
                "branch": "main",
                "spec_folder": "specs/test",
                "cli": "claude",
            },
            headers=auth_headers,
        )
        dispatch_id = resp.json()["dispatch_id"]

        # Write a result with outcome=error
        results_dir = Path(app.state.config.ipc_dir) / "results"
        results_dir.mkdir(parents=True, exist_ok=True)
        result_data = {
            "workflow_id": dispatch_id,
            "phase": "do-task",
            "outcome": "error",
            "signal": "error",
            "exit_code": 1,
            "duration_secs": 5,
            "spec_modified": False,
        }
        result_file = results_dir / f"{dispatch_id}.json"
        result_file.write_text(json.dumps(result_data))

        resp = await client.get(f"/api/v1/dispatch/{dispatch_id}", headers=auth_headers)
        assert resp.status_code == 200
        data = resp.json()
        assert data["status"] == "failed"
        assert data["outcome"] == "error"

    async def test_create_dispatch_no_event_when_daemon_delegated(self, client, auth_headers, app):
        """No DispatchReceived event emitted when delegating to daemon."""
        app.state.execution_env = None
        mock_emitter = AsyncMock()
        app.state.emitter = mock_emitter

        await client.post(
            "/api/v1/dispatch",
            json={
                "project": "test-project",
                "phase": "do-task",
                "branch": "main",
                "spec_folder": "specs/test",
                "cli": "claude",
            },
            headers=auth_headers,
        )
        mock_emitter.emit.assert_not_called()

    async def test_cancel_daemon_dispatch_after_pickup_returns_409(self, client, auth_headers, app):
        """Cancel after daemon pickup returns 409."""
        app.state.execution_env = None

        resp = await client.post(
            "/api/v1/dispatch",
            json={
                "project": "test-project",
                "phase": "do-task",
                "branch": "main",
                "spec_folder": "specs/test",
                "cli": "claude",
            },
            headers=auth_headers,
        )
        dispatch_id = resp.json()["dispatch_id"]

        # Simulate daemon pickup by deleting the IPC file
        store = app.state.api_store
        record = await store.get_dispatch(dispatch_id)
        assert record is not None
        assert record.dispatch_path is not None
        record.dispatch_path.unlink()

        # Cancel — daemon already picked it up
        cancel_resp = await client.delete(f"/api/v1/dispatch/{dispatch_id}", headers=auth_headers)
        assert cancel_resp.status_code == 409
        assert "picked up by the daemon" in cancel_resp.json()["detail"]

    async def test_cancel_pending_transitions_before_cancelling_task(
        self, client, auth_headers, app, monkeypatch
    ):
        """Cancel of a PENDING dispatch transitions before cancelling the task.
        If transition fails (worker raced to RUNNING), task is untouched."""
        store = app.state.api_store

        async def _hang_forever() -> None:
            await asyncio.sleep(3600)

        task = asyncio.create_task(_hang_forever())
        dispatch = Dispatch(
            workflow_id="wf-cancel-race",
            project="test",
            phase=Phase.DO_TASK,
            branch="main",
            spec_folder="specs/test",
            cli=Cli.CLAUDE,
            timeout=1800,
        )
        record = DispatchRecord(
            dispatch_id="wf-cancel-race",
            dispatch=dispatch,
            status=DispatchRunStatus.PENDING,
            created_at="2026-01-01T00:00:00Z",
        )
        record.task = task
        await store.add_dispatch(record)

        # Monkeypatch try_transition_dispatch to return None for PENDING→CANCELLED
        # (simulates worker racing to RUNNING)
        original_transition = store.try_transition_dispatch

        async def _block_pending_cancel(dispatch_id, *, from_statuses, **kwargs):
            if DispatchRunStatus.PENDING in from_statuses:
                return None
            return await original_transition(dispatch_id, from_statuses=from_statuses, **kwargs)

        monkeypatch.setattr(store, "try_transition_dispatch", _block_pending_cancel)

        cancel_resp = await client.delete("/api/v1/dispatch/wf-cancel-race", headers=auth_headers)
        assert cancel_resp.status_code == 409

        # Task must NOT have been cancelled — transition failed before cancel
        assert not task.cancelled()
        assert not task.done()

        # Cleanup
        task.cancel()
        with contextlib.suppress(asyncio.CancelledError):
            await task

    async def test_cancel_pending_local_dispatch_cancels_task(self, client, auth_headers, app):
        """Cancel of a PENDING local dispatch cancels the background task."""
        store = app.state.api_store

        async def _hang_forever() -> None:
            await asyncio.sleep(3600)

        task = asyncio.create_task(_hang_forever())
        dispatch = Dispatch(
            workflow_id="wf-cancel-pending",
            project="test",
            phase=Phase.DO_TASK,
            branch="main",
            spec_folder="specs/test",
            cli=Cli.CLAUDE,
            timeout=1800,
        )
        record = DispatchRecord(
            dispatch_id="wf-cancel-pending",
            dispatch=dispatch,
            status=DispatchRunStatus.PENDING,
            created_at="2026-01-01T00:00:00Z",
        )
        record.task = task
        await store.add_dispatch(record)

        cancel_resp = await client.delete(
            "/api/v1/dispatch/wf-cancel-pending", headers=auth_headers
        )
        assert cancel_resp.status_code == 200
        # Let the event loop process the cancellation
        await asyncio.sleep(0)
        assert task.cancelled() or task.done()

    async def test_create_dispatch_emits_event_when_execution_env_present(
        self, client, auth_headers, app
    ):
        """DispatchReceived event emitted when execution_env is available."""
        mock_emitter = AsyncMock()
        app.state.emitter = mock_emitter

        await client.post(
            "/api/v1/dispatch",
            json={
                "project": "test-project",
                "phase": "do-task",
                "branch": "main",
                "spec_folder": "specs/test",
                "cli": "claude",
            },
            headers=auth_headers,
        )
        mock_emitter.emit.assert_called_once()

    async def test_background_task_exits_early_when_cancelled_before_start(
        self, client, auth_headers, app
    ):
        """Background task exits without provisioning when dispatch is already CANCELLED."""
        from tanren_api.routers.dispatch import _dispatch_background  # noqa: PLC0415,PLC2701

        store = app.state.api_store
        mock_env = app.state.execution_env

        dispatch = Dispatch(
            workflow_id="wf-early-cancel",
            project="test-project",
            phase=Phase.DO_TASK,
            branch="main",
            spec_folder="specs/test",
            cli=Cli.CLAUDE,
            timeout=1800,
        )
        record = DispatchRecord(
            dispatch_id="wf-early-cancel",
            dispatch=dispatch,
            status=DispatchRunStatus.CANCELLED,
            created_at="2026-01-01T00:00:00Z",
            completed_at="2026-01-01T00:00:01Z",
        )
        await store.add_dispatch(record)

        await _dispatch_background(dispatch, "wf-early-cancel", mock_env, app.state.config, store)

        # provision/execute should never have been called
        mock_env.provision.assert_not_called()
        mock_env.execute.assert_not_called()

        # Status should remain CANCELLED
        final = await store.get_dispatch("wf-early-cancel")
        assert final is not None
        assert final.status == DispatchRunStatus.CANCELLED

    async def test_cancel_dispatch_unlink_os_error_returns_409(
        self, client, auth_headers, app, monkeypatch
    ):
        """Cancel returns 409 when IPC file unlink fails with OSError."""
        app.state.execution_env = None

        resp = await client.post(
            "/api/v1/dispatch",
            json={
                "project": "test-project",
                "phase": "do-task",
                "branch": "main",
                "spec_folder": "specs/test",
                "cli": "claude",
            },
            headers=auth_headers,
        )
        dispatch_id = resp.json()["dispatch_id"]

        # Verify dispatch exists with a dispatch_path
        store = app.state.api_store
        record = await store.get_dispatch(dispatch_id)
        assert record is not None
        assert record.dispatch_path is not None

        # Patch Path.unlink to raise PermissionError (subclass of OSError)
        monkeypatch.setattr(
            Path,
            "unlink",
            lambda self, **kw: (_ for _ in ()).throw(PermissionError("denied")),
        )

        cancel_resp = await client.delete(f"/api/v1/dispatch/{dispatch_id}", headers=auth_headers)
        assert cancel_resp.status_code == 409
        assert "could not be cancelled" in cancel_resp.json()["detail"]

        # Status should remain PENDING (not marked CANCELLED)
        final = await store.get_dispatch(dispatch_id)
        assert final is not None
        assert final.status == DispatchRunStatus.PENDING

    async def test_dispatch_teardown_shielded_from_cancellation(self, client, auth_headers, app):
        """Teardown is shielded from cancellation in _dispatch_background."""
        from tanren_api.routers.dispatch import _dispatch_background  # noqa: PLC0415,PLC2701

        store = app.state.api_store
        mock_env = app.state.execution_env

        dispatch = Dispatch(
            workflow_id="wf-shield-test",
            project="test-project",
            phase=Phase.DO_TASK,
            branch="main",
            spec_folder="specs/test",
            cli=Cli.CLAUDE,
            timeout=1800,
        )
        record = DispatchRecord(
            dispatch_id="wf-shield-test",
            dispatch=dispatch,
            status=DispatchRunStatus.PENDING,
            created_at="2026-01-01T00:00:00Z",
        )
        await store.add_dispatch(record)

        # Make execute hang so we can cancel during execution
        execute_started = asyncio.Event()

        async def _hang(*args, **kwargs):
            execute_started.set()
            await asyncio.sleep(3600)

        mock_env.execute = AsyncMock(side_effect=_hang)

        task = asyncio.create_task(
            _dispatch_background(dispatch, "wf-shield-test", mock_env, app.state.config, store)
        )

        # Wait for execute to start, then cancel
        await execute_started.wait()
        task.cancel()
        with contextlib.suppress(asyncio.CancelledError):
            await task

        # Give shielded teardown a tick to complete
        await asyncio.sleep(0)

        mock_env.teardown.assert_awaited_once()

    async def test_cancel_running_dispatch_does_not_overwrite_concurrent_completion(
        self, client, auth_headers, app
    ):
        """Cancel returns 409 when the background task already moved to a terminal status."""
        store = app.state.api_store

        # Create a dispatch in RUNNING state with a completed task
        dispatch = Dispatch(
            workflow_id="wf-race-cancel",
            project="test-project",
            phase=Phase.DO_TASK,
            branch="main",
            spec_folder="specs/test",
            cli=Cli.CLAUDE,
            timeout=1800,
        )
        record = DispatchRecord(
            dispatch_id="wf-race-cancel",
            dispatch=dispatch,
            status=DispatchRunStatus.RUNNING,
            created_at="2026-01-01T00:00:00Z",
            started_at="2026-01-01T00:00:01Z",
        )
        await store.add_dispatch(record)

        # Simulate the background task completing and transitioning to COMPLETED
        await store.try_transition_dispatch(
            "wf-race-cancel",
            from_statuses=frozenset({DispatchRunStatus.RUNNING}),
            to_status=DispatchRunStatus.COMPLETED,
            completed_at="2026-01-01T00:01:00Z",
        )

        # Cancel request should get 409 (terminal status check)
        cancel_resp = await client.delete("/api/v1/dispatch/wf-race-cancel", headers=auth_headers)
        assert cancel_resp.status_code == 409

        # Final status must be COMPLETED, not CANCELLED
        final = await store.get_dispatch("wf-race-cancel")
        assert final is not None
        assert final.status == DispatchRunStatus.COMPLETED

    async def test_background_task_does_not_overwrite_cancelled(self, client, auth_headers, app):
        """Background task completing after cancellation does not overwrite CANCELLED status."""
        from tanren_api.routers.dispatch import _dispatch_background  # noqa: PLC0415,PLC2701

        store = app.state.api_store
        mock_env = app.state.execution_env

        # Create a dispatch in PENDING state
        dispatch = Dispatch(
            workflow_id="wf-race-test",
            project="test-project",
            phase=Phase.DO_TASK,
            branch="main",
            spec_folder="specs/test",
            cli=Cli.CLAUDE,
            timeout=1800,
        )
        record = DispatchRecord(
            dispatch_id="wf-race-test",
            dispatch=dispatch,
            status=DispatchRunStatus.PENDING,
            created_at="2026-01-01T00:00:00Z",
        )
        await store.add_dispatch(record)

        # Simulate cancellation happening during execution: execute's side effect
        # sets the dispatch to CANCELLED (mimicking a concurrent cancel request)
        original_execute = mock_env.execute

        async def _cancel_then_return(*args, **kwargs):
            await store.update_dispatch(
                "wf-race-test",
                status=DispatchRunStatus.CANCELLED,
                completed_at="2026-01-01T00:01:00Z",
            )
            return await original_execute(*args, **kwargs)

        mock_env.execute = AsyncMock(side_effect=_cancel_then_return)

        # Run the background task — it should NOT overwrite CANCELLED
        await _dispatch_background(dispatch, "wf-race-test", mock_env, app.state.config, store)

        final = await store.get_dispatch("wf-race-test")
        assert final is not None
        assert final.status == DispatchRunStatus.CANCELLED
