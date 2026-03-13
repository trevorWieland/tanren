"""Tests for run lifecycle endpoints."""

from __future__ import annotations

import asyncio
import contextlib
from unittest.mock import AsyncMock

import pytest

from tanren_api.models import RunEnvironmentStatus


@pytest.mark.api
class TestRun:
    async def test_provision_returns_environment(self, client, auth_headers):
        resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        assert resp.status_code == 200
        data = resp.json()
        assert "env_id" in data
        assert "vm_id" in data
        assert "host" in data
        assert data["status"] == "provisioned"

    async def test_provision_no_execution_env(self, client, auth_headers, app):
        app.state.execution_env = None
        resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        assert resp.status_code == 500

    async def test_execute_returns_accepted(self, client, auth_headers, app):
        # Provision first
        prov_resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        env_id = prov_resp.json()["env_id"]

        resp = await client.post(
            f"/api/v1/run/{env_id}/execute",
            json={
                "project": "test",
                "spec_path": "specs/test",
                "phase": "do-task",
            },
            headers=auth_headers,
        )
        assert resp.status_code == 200
        data = resp.json()
        assert data["env_id"] == env_id
        assert "dispatch_id" in data
        assert data["status"] == "executing"

    async def test_execute_env_not_found(self, client, auth_headers):
        resp = await client.post(
            "/api/v1/run/nonexistent/execute",
            json={
                "project": "test",
                "spec_path": "specs/test",
                "phase": "do-task",
            },
            headers=auth_headers,
        )
        assert resp.status_code == 404

    async def test_teardown_returns_accepted(self, client, auth_headers):
        # Provision first
        prov_resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        env_id = prov_resp.json()["env_id"]

        resp = await client.post(
            f"/api/v1/run/{env_id}/teardown",
            headers=auth_headers,
        )
        assert resp.status_code == 200
        data = resp.json()
        assert data["env_id"] == env_id
        assert data["status"] == "tearing_down"

    async def test_status_returns_status(self, client, auth_headers):
        # Provision first
        prov_resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        env_id = prov_resp.json()["env_id"]

        resp = await client.get(
            f"/api/v1/run/{env_id}/status",
            headers=auth_headers,
        )
        assert resp.status_code == 200
        data = resp.json()
        assert data["env_id"] == env_id
        assert data["status"] == "provisioned"

    async def test_status_not_found(self, client, auth_headers):
        resp = await client.get("/api/v1/run/nonexistent/status", headers=auth_headers)
        assert resp.status_code == 404

    async def test_execute_no_execution_env(self, client, auth_headers, app):
        # Provision first (with execution_env available)
        prov_resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        env_id = prov_resp.json()["env_id"]

        # Now remove execution_env
        app.state.execution_env = None

        resp = await client.post(
            f"/api/v1/run/{env_id}/execute",
            json={
                "project": "test",
                "spec_path": "specs/test",
                "phase": "do-task",
            },
            headers=auth_headers,
        )
        assert resp.status_code == 500

    async def test_execute_project_mismatch_returns_409(self, client, auth_headers):
        """Execute with a different project than provisioned returns 409."""
        prov_resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        env_id = prov_resp.json()["env_id"]

        resp = await client.post(
            f"/api/v1/run/{env_id}/execute",
            json={
                "project": "wrong-project",
                "spec_path": "specs/test",
                "phase": "do-task",
            },
            headers=auth_headers,
        )
        assert resp.status_code == 409

    async def test_teardown_awaits_cancelled_execute_task(self, client, auth_headers):
        """Teardown waits for cancelled execute task before proceeding."""
        # Provision
        prov_resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        env_id = prov_resp.json()["env_id"]

        # Execute
        await client.post(
            f"/api/v1/run/{env_id}/execute",
            json={
                "project": "test",
                "spec_path": "specs/test",
                "phase": "do-task",
            },
            headers=auth_headers,
        )

        # Teardown — should succeed even with a running execute task
        resp = await client.post(
            f"/api/v1/run/{env_id}/teardown",
            headers=auth_headers,
        )
        assert resp.status_code == 200
        data = resp.json()
        assert data["env_id"] == env_id
        assert data["status"] == "tearing_down"

    async def test_execute_on_tearing_down_env_returns_409(self, client, auth_headers, app):
        """Execute on a tearing-down environment returns 409."""
        # Provision
        prov_resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        env_id = prov_resp.json()["env_id"]

        # Set status to TEARING_DOWN directly to avoid background removal race
        store = app.state.api_store
        await store.update_environment(env_id, status=RunEnvironmentStatus.TEARING_DOWN)

        # Attempt execute — should be 409
        resp = await client.post(
            f"/api/v1/run/{env_id}/execute",
            json={
                "project": "test",
                "spec_path": "specs/test",
                "phase": "do-task",
            },
            headers=auth_headers,
        )
        assert resp.status_code == 409

    async def test_provision_wraps_provider_exception(
        self, client, auth_headers, mock_execution_env
    ):
        """run_provision wraps provider exceptions in ServiceError (500)."""
        mock_execution_env.provision = AsyncMock(side_effect=RuntimeError("boom"))

        resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        assert resp.status_code == 500
        data = resp.json()
        assert data["error_code"] == "service_error"
        assert "boom" not in data["detail"]

    async def test_teardown_no_execution_env_returns_500(self, client, auth_headers, app):
        """Teardown without execution_env returns 500 and preserves environment record."""
        # Provision first (with execution_env available)
        prov_resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        env_id = prov_resp.json()["env_id"]

        # Remove execution_env
        app.state.execution_env = None

        resp = await client.post(
            f"/api/v1/run/{env_id}/teardown",
            headers=auth_headers,
        )
        assert resp.status_code == 500

        # Environment record should still exist
        store = app.state.api_store
        record = await store.get_environment(env_id)
        assert record is not None

    async def test_teardown_returns_409_when_task_cannot_be_stopped(
        self, client, auth_headers, app, monkeypatch
    ):
        """Teardown returns 409 when a running task cannot be stopped."""
        # Provision first
        prov_resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        env_id = prov_resp.json()["env_id"]

        # Attach a task that resists one cancellation attempt
        async def _resist_one_cancel() -> None:
            try:
                await asyncio.sleep(3600)
            except asyncio.CancelledError:
                asyncio.current_task().uncancel()  # type: ignore[union-attr]
                await asyncio.sleep(3600)

        task = asyncio.create_task(_resist_one_cancel())
        store = app.state.api_store
        await store.update_environment(env_id, task=task)
        await asyncio.sleep(0)  # Let task enter its try block

        # Reduce wait_secs to avoid 5s delay in tests
        original_cancel = store.cancel_environment_task

        async def _fast_cancel(eid: str, *, wait_secs: float = 0.1) -> bool:
            return await original_cancel(eid, wait_secs=wait_secs)

        monkeypatch.setattr(store, "cancel_environment_task", _fast_cancel)

        resp = await client.post(
            f"/api/v1/run/{env_id}/teardown",
            headers=auth_headers,
        )
        assert resp.status_code == 409
        assert "could not be stopped" in resp.json()["detail"]

        # Cleanup: cancel again (this time it propagates)
        task.cancel()
        with contextlib.suppress(asyncio.CancelledError):
            await task

    async def test_re_execute_clears_stale_outcome(
        self, client, auth_headers, app, mock_execution_env
    ):
        """Re-executing a COMPLETED env clears outcome and completed_at."""
        from tanren_core.schemas import Outcome  # noqa: PLC0415

        # Provision
        prov_resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        env_id = prov_resp.json()["env_id"]

        # Simulate a completed first execution
        store = app.state.api_store
        await store.update_environment(
            env_id,
            status=RunEnvironmentStatus.COMPLETED,
            outcome=Outcome.SUCCESS,
            completed_at="2026-01-01T01:00:00Z",
        )

        # Make execute block so we can observe the EXECUTING state
        execute_event = asyncio.Event()
        original_execute = mock_execution_env.execute

        async def _blocking_execute(*args, **kwargs):
            await execute_event.wait()
            return await original_execute(*args, **kwargs)

        mock_execution_env.execute = AsyncMock(side_effect=_blocking_execute)

        # Re-execute
        resp = await client.post(
            f"/api/v1/run/{env_id}/execute",
            json={
                "project": "test",
                "spec_path": "specs/test",
                "phase": "do-task",
            },
            headers=auth_headers,
        )
        assert resp.status_code == 200

        # Poll status — outcome and completed_at should be cleared
        status_resp = await client.get(
            f"/api/v1/run/{env_id}/status",
            headers=auth_headers,
        )
        assert status_resp.status_code == 200
        data = status_resp.json()
        assert data["status"] == "executing"
        assert data["outcome"] is None

        # Unblock the background task so it doesn't leak
        execute_event.set()
        await asyncio.sleep(0)

    async def test_full_returns_accepted(self, client, auth_headers):
        resp = await client.post(
            "/api/v1/run/full",
            json={
                "project": "test",
                "branch": "main",
                "spec_path": "specs/test",
                "phase": "do-task",
            },
            headers=auth_headers,
        )
        assert resp.status_code == 200
        data = resp.json()
        assert "dispatch_id" in data
        assert data["status"] == "accepted"

    async def test_full_lifecycle_teardown_shielded_from_cancellation(
        self, client, auth_headers, app, mock_execution_env
    ):
        """Teardown in _full_lifecycle is shielded from cancellation."""
        store = app.state.api_store

        # Make execute hang so we can cancel during execution
        execute_started = asyncio.Event()

        async def _hang(*args, **kwargs):
            execute_started.set()
            await asyncio.sleep(3600)

        mock_execution_env.execute = AsyncMock(side_effect=_hang)

        resp = await client.post(
            "/api/v1/run/full",
            json={
                "project": "test",
                "branch": "main",
                "spec_path": "specs/test",
                "phase": "do-task",
            },
            headers=auth_headers,
        )
        assert resp.status_code == 200
        dispatch_id = resp.json()["dispatch_id"]

        # Wait for execute to start
        await execute_started.wait()

        # Cancel the background task
        record = await store.get_dispatch(dispatch_id)
        assert record is not None
        assert record.task is not None
        record.task.cancel()
        with contextlib.suppress(asyncio.CancelledError):
            await record.task

        # Give shielded teardown a tick to complete
        await asyncio.sleep(0)

        mock_execution_env.teardown.assert_awaited_once()
