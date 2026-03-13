"""Tests for dispatch endpoints."""

import json
import re
from pathlib import Path
from unittest.mock import AsyncMock

import pytest


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
