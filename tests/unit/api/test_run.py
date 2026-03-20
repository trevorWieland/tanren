"""Tests for run lifecycle endpoints."""

from __future__ import annotations

import asyncio
import contextlib
from unittest.mock import AsyncMock

import pytest

from tanren_api.models import RunEnvironmentStatus


async def _wait_provisioned(client, env_id, auth_headers, *, max_secs: float = 2.0):
    """Poll until env reaches 'provisioned' status."""
    deadline = asyncio.get_event_loop().time() + max_secs
    while asyncio.get_event_loop().time() < deadline:
        await asyncio.sleep(0.02)
        r = await client.get(f"/api/v1/run/{env_id}/status", headers=auth_headers)
        if r.json()["status"] == "provisioned":
            return
    raise AssertionError(f"Environment {env_id} did not reach 'provisioned' within {max_secs}s")


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
        assert data["status"] == "provisioning"

        # Wait for background provision to complete
        await _wait_provisioned(client, data["env_id"], auth_headers)
        status = await client.get(f"/api/v1/run/{data['env_id']}/status", headers=auth_headers)
        assert status.json()["status"] == "provisioned"

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
        await _wait_provisioned(client, env_id, auth_headers)

        resp = await client.post(
            f"/api/v1/run/{env_id}/execute",
            json={
                "project": "test",
                "spec_path": "specs/test",
                "phase": "do-task",
                "cli": "claude",
                "auth": "api_key",
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
                "cli": "claude",
                "auth": "api_key",
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
        await _wait_provisioned(client, env_id, auth_headers)

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
        await _wait_provisioned(client, env_id, auth_headers)

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
        await _wait_provisioned(client, env_id, auth_headers)

        # Now remove execution_env
        app.state.execution_env = None

        resp = await client.post(
            f"/api/v1/run/{env_id}/execute",
            json={
                "project": "test",
                "spec_path": "specs/test",
                "phase": "do-task",
                "cli": "claude",
                "auth": "api_key",
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
        await _wait_provisioned(client, env_id, auth_headers)

        resp = await client.post(
            f"/api/v1/run/{env_id}/execute",
            json={
                "project": "wrong-project",
                "spec_path": "specs/test",
                "phase": "do-task",
                "cli": "claude",
                "auth": "api_key",
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
        await _wait_provisioned(client, env_id, auth_headers)

        # Execute
        await client.post(
            f"/api/v1/run/{env_id}/execute",
            json={
                "project": "test",
                "spec_path": "specs/test",
                "phase": "do-task",
                "cli": "claude",
                "auth": "api_key",
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
        await _wait_provisioned(client, env_id, auth_headers)

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
                "cli": "claude",
                "auth": "api_key",
            },
            headers=auth_headers,
        )
        assert resp.status_code == 409

    async def test_provision_wraps_provider_exception(
        self, client, auth_headers, mock_execution_env
    ):
        """run_provision surfaces provider failure via status=failed."""
        mock_execution_env.provision = AsyncMock(side_effect=RuntimeError("boom"))

        resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        # Returns 200 immediately (non-blocking)
        assert resp.status_code == 200
        env_id = resp.json()["env_id"]

        # Wait for background task to fail
        await asyncio.sleep(0.05)

        status_resp = await client.get(f"/api/v1/run/{env_id}/status", headers=auth_headers)
        assert status_resp.json()["status"] == "failed"

    async def test_teardown_no_execution_env_returns_500(self, client, auth_headers, app):
        """Teardown without execution_env returns 500 and preserves environment record."""
        # Provision first (with execution_env available)
        prov_resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        env_id = prov_resp.json()["env_id"]
        await _wait_provisioned(client, env_id, auth_headers)

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

    async def test_teardown_proceeds_when_task_resists_cancel(
        self, client, auth_headers, app, monkeypatch
    ):
        """Teardown succeeds even when a running task resists cancellation,
        because ownership is claimed via transition before cancel."""
        # Provision first
        prov_resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        env_id = prov_resp.json()["env_id"]
        await _wait_provisioned(client, env_id, auth_headers)

        # Attach a task that resists one cancellation attempt
        async def _resist_one_cancel() -> None:
            try:
                await asyncio.sleep(3600)
            except asyncio.CancelledError:
                task = asyncio.current_task()
                assert task is not None
                task.uncancel()
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
        # Teardown claims ownership first, so it succeeds regardless of cancel result
        assert resp.status_code == 200
        assert resp.json()["status"] == "tearing_down"

        # Cleanup: cancel again (this time it propagates)
        task.cancel()
        with contextlib.suppress(asyncio.CancelledError):
            await task

    async def test_teardown_transitions_before_cancelling_task(
        self, client, auth_headers, app, mock_execution_env
    ):
        """First teardown succeeds and second gets 409 without killing first's task."""
        # Provision
        prov_resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        env_id = prov_resp.json()["env_id"]
        await _wait_provisioned(client, env_id, auth_headers)

        # Make teardown block so first teardown task stays alive
        teardown_event = asyncio.Event()

        async def _blocking_teardown(*args, **kwargs):
            await teardown_event.wait()

        mock_execution_env.teardown = AsyncMock(side_effect=_blocking_teardown)

        # First teardown — should succeed
        resp1 = await client.post(
            f"/api/v1/run/{env_id}/teardown",
            headers=auth_headers,
        )
        assert resp1.status_code == 200

        # Get the teardown task
        store = app.state.api_store
        record = await store.get_environment(env_id)
        assert record is not None
        teardown_task = record.task
        assert teardown_task is not None
        assert not teardown_task.done()

        # Second teardown — should get 409
        resp2 = await client.post(
            f"/api/v1/run/{env_id}/teardown",
            headers=auth_headers,
        )
        assert resp2.status_code == 409
        assert "already being torn down" in resp2.json()["detail"]

        # First teardown task must NOT have been cancelled
        assert not teardown_task.cancelled()
        assert not teardown_task.done()

        # Cleanup
        teardown_event.set()
        await asyncio.sleep(0)

    async def test_teardown_background_shielded_from_cancellation(
        self, client, auth_headers, app, mock_execution_env
    ):
        """Teardown background task shields execution_env.teardown from cancellation
        and waits for it to finish before proceeding to remove_environment."""
        # Provision
        prov_resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        env_id = prov_resp.json()["env_id"]
        await _wait_provisioned(client, env_id, auth_headers)

        # Make teardown block on an event so we control when it finishes
        teardown_started = asyncio.Event()
        teardown_finish = asyncio.Event()

        async def _blocking_teardown(*args, **kwargs):
            teardown_started.set()
            await teardown_finish.wait()

        mock_execution_env.teardown = AsyncMock(side_effect=_blocking_teardown)

        # Start teardown
        resp = await client.post(
            f"/api/v1/run/{env_id}/teardown",
            headers=auth_headers,
        )
        assert resp.status_code == 200

        # Wait for teardown to start
        await teardown_started.wait()

        # Get the background task and cancel it (simulating shutdown)
        store = app.state.api_store
        record = await store.get_environment(env_id)
        assert record is not None
        bg_task = record.task
        assert bg_task is not None
        bg_task.cancel()
        await asyncio.sleep(0)  # Let cancellation propagate

        # Background task must NOT be done yet — it's waiting for inner teardown
        assert not bg_task.done(), "Task returned before teardown finished"

        # Environment record should still exist (remove_environment not called yet)
        assert await store.get_environment(env_id) is not None

        # Now unblock teardown
        teardown_finish.set()
        with contextlib.suppress(asyncio.CancelledError):
            await bg_task

        # After teardown completes, environment should be removed
        assert await store.get_environment(env_id) is None
        mock_execution_env.teardown.assert_awaited_once()

    async def test_re_execute_clears_stale_outcome(
        self, client, auth_headers, app, mock_execution_env
    ):
        """Re-executing a COMPLETED env clears outcome and completed_at."""
        from tanren_core.schemas import Outcome

        # Provision
        prov_resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        env_id = prov_resp.json()["env_id"]
        await _wait_provisioned(client, env_id, auth_headers)

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
                "cli": "claude",
                "auth": "api_key",
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
                "cli": "claude",
                "auth": "api_key",
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
        """Teardown in _full_lifecycle is shielded from cancellation
        and the task doesn't return until teardown actually finishes."""
        store = app.state.api_store

        # Make execute hang so we can cancel during execution
        execute_started = asyncio.Event()

        async def _hang_execute(*args, **kwargs):
            execute_started.set()
            await asyncio.sleep(3600)

        mock_execution_env.execute = AsyncMock(side_effect=_hang_execute)

        # Make teardown block on an event so we control when it finishes
        teardown_started = asyncio.Event()
        teardown_finish = asyncio.Event()

        async def _blocking_teardown(*args, **kwargs):
            teardown_started.set()
            await teardown_finish.wait()

        mock_execution_env.teardown = AsyncMock(side_effect=_blocking_teardown)

        resp = await client.post(
            "/api/v1/run/full",
            json={
                "project": "test",
                "branch": "main",
                "spec_path": "specs/test",
                "phase": "do-task",
                "cli": "claude",
                "auth": "api_key",
            },
            headers=auth_headers,
        )
        assert resp.status_code == 200
        dispatch_id = resp.json()["dispatch_id"]

        # Wait for execute to start
        await execute_started.wait()

        # Cancel the background task (simulating shutdown)
        record = await store.get_dispatch(dispatch_id)
        assert record is not None
        bg_task = record.task
        assert bg_task is not None
        bg_task.cancel()

        # Wait for teardown to start (the finally block shields it)
        await teardown_started.wait()

        # Background task must NOT be done yet — it's waiting for inner teardown
        assert not bg_task.done(), "Task returned before teardown finished"

        # Now unblock teardown
        teardown_finish.set()
        with contextlib.suppress(asyncio.CancelledError):
            await bg_task

        mock_execution_env.teardown.assert_awaited_once()

    async def test_execute_task_registered_atomically_with_transition(
        self, client, auth_headers, app, mock_execution_env
    ):
        """Task is present on the environment record as soon as status is EXECUTING."""
        # Provision
        prov_resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        env_id = prov_resp.json()["env_id"]
        await _wait_provisioned(client, env_id, auth_headers)

        # Make execute block so we can inspect state while EXECUTING
        execute_event = asyncio.Event()
        original_execute = mock_execution_env.execute

        async def _blocking_execute(*args, **kwargs):
            await execute_event.wait()
            return await original_execute(*args, **kwargs)

        mock_execution_env.execute = AsyncMock(side_effect=_blocking_execute)

        # Start execute
        resp = await client.post(
            f"/api/v1/run/{env_id}/execute",
            json={
                "project": "test",
                "spec_path": "specs/test",
                "phase": "do-task",
                "cli": "claude",
                "auth": "api_key",
            },
            headers=auth_headers,
        )
        assert resp.status_code == 200

        # The task should already be on the record — no gap
        store = app.state.api_store
        record = await store.get_environment(env_id)
        assert record is not None
        assert record.status == RunEnvironmentStatus.EXECUTING
        assert record.task is not None
        assert not record.task.done()

        # Cleanup
        execute_event.set()
        await asyncio.sleep(0)

    async def test_second_teardown_does_not_cancel_first_teardown_task(
        self, client, auth_headers, app
    ):
        """A second teardown returns 409 without cancelling the in-flight teardown task."""
        # Provision
        prov_resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        env_id = prov_resp.json()["env_id"]
        await _wait_provisioned(client, env_id, auth_headers)

        # Simulate an in-progress teardown: set status to TEARING_DOWN and attach a task
        store = app.state.api_store
        await store.update_environment(env_id, status=RunEnvironmentStatus.TEARING_DOWN)

        async def _long_teardown() -> None:
            await asyncio.sleep(3600)

        teardown_task = asyncio.create_task(_long_teardown())
        await store.update_environment(env_id, task=teardown_task)
        await asyncio.sleep(0)  # Let task start

        # Second teardown should get 409 without killing the first teardown task
        resp = await client.post(
            f"/api/v1/run/{env_id}/teardown",
            headers=auth_headers,
        )
        assert resp.status_code == 409
        assert "already being torn down" in resp.json()["detail"]

        # The first teardown task must NOT have been cancelled
        assert not teardown_task.cancelled()
        assert not teardown_task.done()

        # Cleanup
        teardown_task.cancel()
        with contextlib.suppress(asyncio.CancelledError):
            await teardown_task

    async def test_concurrent_teardown_during_execute_startup_sees_task(
        self, client, auth_headers, app, mock_execution_env
    ):
        """Teardown during execute startup finds and cancels the task."""
        # Provision
        prov_resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        env_id = prov_resp.json()["env_id"]
        await _wait_provisioned(client, env_id, auth_headers)

        # Make execute block on an event
        execute_event = asyncio.Event()
        original_execute = mock_execution_env.execute

        async def _blocking_execute(*args, **kwargs):
            await execute_event.wait()
            return await original_execute(*args, **kwargs)

        mock_execution_env.execute = AsyncMock(side_effect=_blocking_execute)

        # Start execute
        resp = await client.post(
            f"/api/v1/run/{env_id}/execute",
            json={
                "project": "test",
                "spec_path": "specs/test",
                "phase": "do-task",
                "cli": "claude",
                "auth": "api_key",
            },
            headers=auth_headers,
        )
        assert resp.status_code == 200

        # Now teardown — should find the task and cancel it
        teardown_resp = await client.post(
            f"/api/v1/run/{env_id}/teardown",
            headers=auth_headers,
        )
        assert teardown_resp.status_code == 200
        assert teardown_resp.json()["status"] == "tearing_down"

    async def test_teardown_uses_fresh_handle_from_transition(
        self, client, auth_headers, app, mock_execution_env
    ):
        """Teardown uses the transitioned record's handle, not the stale snapshot."""
        from tanren_api.state import (
            EnvironmentRecord,
        )

        store = app.state.api_store

        # Manually add an environment with handle=None (simulating a stale snapshot)
        env_id = "env-stale-handle"
        stale_record = EnvironmentRecord(
            env_id=env_id,
            handle=None,
            status=RunEnvironmentStatus.PROVISIONED,
            started_at="2026-01-01T00:00:00Z",
        )
        await store.add_environment(stale_record)

        # Now update the store with a real handle (simulating provision completing)
        handle = mock_execution_env.provision.return_value
        await store.update_environment(env_id, handle=handle, vm_id="vm-1", host="10.0.0.1")

        # Teardown — should use the handle from the transitioned record
        resp = await client.post(
            f"/api/v1/run/{env_id}/teardown",
            headers=auth_headers,
        )
        assert resp.status_code == 200

        # Wait for background teardown to complete
        await asyncio.sleep(0.1)

        # execution_env.teardown must have been called with the real handle
        mock_execution_env.teardown.assert_awaited_once_with(handle)

    async def test_status_includes_vm_id_and_host(self, client, auth_headers, app):
        """GET /run/{env_id}/status includes vm_id and host after provisioning."""
        # Provision
        prov_resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        env_id = prov_resp.json()["env_id"]
        await _wait_provisioned(client, env_id, auth_headers)

        resp = await client.get(
            f"/api/v1/run/{env_id}/status",
            headers=auth_headers,
        )
        assert resp.status_code == 200
        data = resp.json()
        assert data["vm_id"] == "vm-test-1"
        assert data["host"] == "10.0.0.1"

    async def test_provision_cancelled_after_handle_triggers_cleanup(
        self, client, auth_headers, app, mock_execution_env, monkeypatch
    ):
        """Provision cancelled after handle obtained → finally block calls teardown."""
        from tanren_api.state import (
            _UNSET,
        )

        store = app.state.api_store
        original_handle = mock_execution_env.provision.return_value

        # Block try_transition_environment when it tries to persist the handle,
        # giving us a window to cancel the task.
        persist_reached = asyncio.Event()
        persist_release = asyncio.Event()
        original_try = store.try_transition_environment

        async def _intercept_try(env_id, **kwargs):
            h = kwargs.get("handle", _UNSET)
            if not isinstance(h, type(_UNSET)) and h is not None:
                persist_reached.set()
                await persist_release.wait()
            return await original_try(env_id, **kwargs)

        monkeypatch.setattr(store, "try_transition_environment", _intercept_try)

        resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        assert resp.status_code == 200
        env_id = resp.json()["env_id"]

        # Wait for provision to reach the persist step
        await persist_reached.wait()

        # Cancel the task while it's blocked before persisting handle
        record = await store.get_environment(env_id)
        assert record is not None
        bg_task = record.task
        assert bg_task is not None
        bg_task.cancel()

        # Let the intercepted update proceed — task sees CancelledError
        persist_release.set()
        with contextlib.suppress(asyncio.CancelledError):
            await bg_task

        # Finally block should have called teardown with the handle
        mock_execution_env.teardown.assert_awaited_once_with(original_handle)

    async def test_teardown_rereads_handle_from_store(
        self, client, auth_headers, app, mock_execution_env
    ):
        """Teardown re-reads handle from store, picking up a handle that was
        persisted after the transition snapshot was taken."""
        from tanren_api.state import (
            EnvironmentRecord,
        )

        store = app.state.api_store
        handle = mock_execution_env.provision.return_value

        # Create env with handle=None (simulating transition snapshot)
        env_id = "env-reread-handle"
        record = EnvironmentRecord(
            env_id=env_id,
            handle=None,
            status=RunEnvironmentStatus.PROVISIONED,
            started_at="2026-01-01T00:00:00Z",
        )
        await store.add_environment(record)

        # Provision completes and persists the handle (Window B scenario)
        await store.update_environment(env_id, handle=handle, vm_id="vm-1", host="10.0.0.1")

        # Teardown — should re-read and find the persisted handle
        resp = await client.post(
            f"/api/v1/run/{env_id}/teardown",
            headers=auth_headers,
        )
        assert resp.status_code == 200

        # Wait for background teardown
        await asyncio.sleep(0.1)

        # teardown must have been called with the real handle
        mock_execution_env.teardown.assert_awaited_once_with(handle)

    async def test_teardown_removes_record_when_provision_cleaned_up(
        self, client, auth_headers, app, mock_execution_env
    ):
        """When provision was cancelled and cleaned up its own handle,
        teardown re-reads handle=None and just removes the record."""
        from tanren_api.state import (
            EnvironmentRecord,
        )

        store = app.state.api_store

        # Env with handle=None — provision's finally block already cleaned up the VM
        env_id = "env-already-cleaned"
        record = EnvironmentRecord(
            env_id=env_id,
            handle=None,
            status=RunEnvironmentStatus.PROVISIONING,
            started_at="2026-01-01T00:00:00Z",
        )
        await store.add_environment(record)

        # Teardown — should just remove the record
        resp = await client.post(
            f"/api/v1/run/{env_id}/teardown",
            headers=auth_headers,
        )
        assert resp.status_code == 200

        # Wait for background teardown
        await asyncio.sleep(0.1)

        # Record should be removed
        assert await store.get_environment(env_id) is None
        # teardown should NOT have been called (no handle)
        mock_execution_env.teardown.assert_not_awaited()

    async def test_status_vm_id_null_before_provisioned(
        self, client, auth_headers, app, mock_execution_env
    ):
        """GET /run/{env_id}/status returns null vm_id/host while provisioning."""
        # Make provision hang
        provision_started = asyncio.Event()
        provision_release = asyncio.Event()

        async def _slow_provision(*args, **kwargs):
            provision_started.set()
            await provision_release.wait()
            return mock_execution_env.provision.return_value

        mock_execution_env.provision = AsyncMock(side_effect=_slow_provision)

        prov_resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        env_id = prov_resp.json()["env_id"]
        await provision_started.wait()

        resp = await client.get(
            f"/api/v1/run/{env_id}/status",
            headers=auth_headers,
        )
        assert resp.status_code == 200
        data = resp.json()
        assert data["vm_id"] is None
        assert data["host"] is None

        # Cleanup
        provision_release.set()
        await asyncio.sleep(0.05)

    async def test_teardown_waits_for_noncancellable_provision(
        self, client, auth_headers, app, mock_execution_env, monkeypatch
    ):
        """Teardown waits for a non-cancellable provision task to finish,
        then tears down the handle that provision persisted."""
        store = app.state.api_store
        original_handle = mock_execution_env.provision.return_value

        # Make provision block in a "non-cancellable" section: it suppresses
        # the first CancelledError and then persists the handle normally.
        provision_entered = asyncio.Event()
        provision_release = asyncio.Event()

        async def _noncancellable_provision(*args, **kwargs):
            provision_entered.set()
            try:
                await provision_release.wait()
            except asyncio.CancelledError:
                # Resist cancellation — simulate a provider call that
                # cannot be interrupted (e.g. Hetzner API).
                task = asyncio.current_task()
                assert task is not None
                task.uncancel()
                await provision_release.wait()
            return original_handle

        mock_execution_env.provision = AsyncMock(side_effect=_noncancellable_provision)

        # Reduce cancel_environment_task timeout to avoid slow tests
        original_cancel = store.cancel_environment_task

        async def _fast_cancel(eid: str, *, wait_secs: float = 0.1) -> bool:
            return await original_cancel(eid, wait_secs=wait_secs)

        monkeypatch.setattr(store, "cancel_environment_task", _fast_cancel)

        # Start provision
        prov_resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        assert prov_resp.status_code == 200
        env_id = prov_resp.json()["env_id"]
        await provision_entered.wait()

        # Teardown while provision is still running
        resp = await client.post(
            f"/api/v1/run/{env_id}/teardown",
            headers=auth_headers,
        )
        assert resp.status_code == 200

        # Unblock provision — it finishes and persists the handle
        provision_release.set()

        # Wait for the teardown background task to complete
        await asyncio.sleep(0.2)

        # Teardown should have been called with the handle that provision persisted
        mock_execution_env.teardown.assert_awaited_once_with(original_handle)

        # Environment record should be removed
        assert await store.get_environment(env_id) is None

    async def test_teardown_during_provision_cancel_succeeds_provision_cleans_up(
        self, client, auth_headers, app, mock_execution_env, monkeypatch
    ):
        """Provision cancelled successfully → its finally block cleans up.
        Teardown awaits the prior task, reads handle=None, removes record.
        execution_env.teardown called exactly once (by provision's finally)."""
        store = app.state.api_store
        original_handle = mock_execution_env.provision.return_value

        # Make provision block so we can cancel it; it does NOT resist cancellation.
        provision_entered = asyncio.Event()

        async def _cancellable_provision(*args, **kwargs):
            provision_entered.set()
            await asyncio.sleep(3600)  # Will be cancelled
            return original_handle

        mock_execution_env.provision = AsyncMock(side_effect=_cancellable_provision)

        # Reduce cancel_environment_task timeout
        original_cancel = store.cancel_environment_task

        async def _fast_cancel(eid: str, *, wait_secs: float = 0.1) -> bool:
            return await original_cancel(eid, wait_secs=wait_secs)

        monkeypatch.setattr(store, "cancel_environment_task", _fast_cancel)

        # Start provision
        prov_resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        assert prov_resp.status_code == 200
        env_id = prov_resp.json()["env_id"]
        await provision_entered.wait()

        # Teardown while provision is still running — cancel succeeds this time
        resp = await client.post(
            f"/api/v1/run/{env_id}/teardown",
            headers=auth_headers,
        )
        assert resp.status_code == 200

        # Wait for teardown background task
        await asyncio.sleep(0.2)

        # Provision was cancelled before obtaining a handle, so teardown
        # should NOT have been called by _teardown_background (handle=None).
        # Provision's finally block also has handle=None (never returned),
        # so teardown is never called at all.
        mock_execution_env.teardown.assert_not_awaited()

        # Environment record should be removed
        assert await store.get_environment(env_id) is None

    async def test_provision_does_not_overwrite_tearing_down_status(
        self, client, auth_headers, app, mock_execution_env, monkeypatch
    ):
        """Provision completing after teardown transitions to TEARING_DOWN must
        not overwrite the status back to PROVISIONED.  Provision's finally block
        cleans up the handle instead."""
        store = app.state.api_store
        original_handle = mock_execution_env.provision.return_value

        # Make provision block so we can interleave teardown
        provision_entered = asyncio.Event()
        provision_release = asyncio.Event()

        async def _blocking_provision(*args, **kwargs):
            provision_entered.set()
            try:
                await provision_release.wait()
            except asyncio.CancelledError:
                # Resist cancellation — simulate non-cancellable provider
                task = asyncio.current_task()
                assert task is not None
                task.uncancel()
                await provision_release.wait()
            return original_handle

        mock_execution_env.provision = AsyncMock(side_effect=_blocking_provision)

        # Reduce cancel_environment_task timeout
        original_cancel = store.cancel_environment_task

        async def _fast_cancel(eid: str, *, wait_secs: float = 0.1) -> bool:
            return await original_cancel(eid, wait_secs=wait_secs)

        monkeypatch.setattr(store, "cancel_environment_task", _fast_cancel)

        # Start provision
        prov_resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        assert prov_resp.status_code == 200
        env_id = prov_resp.json()["env_id"]
        await provision_entered.wait()

        # Teardown while provision is blocked — transitions to TEARING_DOWN
        resp = await client.post(
            f"/api/v1/run/{env_id}/teardown",
            headers=auth_headers,
        )
        assert resp.status_code == 200

        # Verify status is TEARING_DOWN before releasing provision
        record = await store.get_environment(env_id)
        assert record is not None
        assert record.status == RunEnvironmentStatus.TEARING_DOWN

        # Release provision — it completes and tries to transition
        provision_release.set()
        await asyncio.sleep(0.3)

        # teardown should have been called exactly once (by provision's finally block)
        mock_execution_env.teardown.assert_awaited_once_with(original_handle)

        # Environment record should be removed by teardown background task
        assert await store.get_environment(env_id) is None

    async def test_provision_error_does_not_overwrite_tearing_down_status(
        self, client, auth_headers, app, mock_execution_env, monkeypatch
    ):
        """Provision raising after teardown transitions to TEARING_DOWN must
        not overwrite the status to FAILED."""
        store = app.state.api_store

        # Make provision block then raise
        provision_entered = asyncio.Event()
        provision_release = asyncio.Event()

        async def _failing_provision(*args, **kwargs):
            provision_entered.set()
            try:
                await provision_release.wait()
            except asyncio.CancelledError:
                task = asyncio.current_task()
                assert task is not None
                task.uncancel()
                await provision_release.wait()
            raise RuntimeError("provider error")

        mock_execution_env.provision = AsyncMock(side_effect=_failing_provision)

        # Reduce cancel_environment_task timeout
        original_cancel = store.cancel_environment_task

        async def _fast_cancel(eid: str, *, wait_secs: float = 0.1) -> bool:
            return await original_cancel(eid, wait_secs=wait_secs)

        monkeypatch.setattr(store, "cancel_environment_task", _fast_cancel)

        # Start provision
        prov_resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        assert prov_resp.status_code == 200
        env_id = prov_resp.json()["env_id"]
        await provision_entered.wait()

        # Teardown while provision is blocked
        resp = await client.post(
            f"/api/v1/run/{env_id}/teardown",
            headers=auth_headers,
        )
        assert resp.status_code == 200

        # Release provision — it raises RuntimeError
        provision_release.set()
        await asyncio.sleep(0.3)

        # teardown should NOT have been called (no handle obtained)
        mock_execution_env.teardown.assert_not_awaited()

        # Environment record should be removed by teardown background task
        assert await store.get_environment(env_id) is None

    async def test_teardown_does_not_hang_on_stuck_prior_task(
        self, client, auth_headers, app, mock_execution_env, monkeypatch
    ):
        """Prior task that hangs forever does not block teardown indefinitely."""
        import tanren_api.services.run as run_module

        store = app.state.api_store

        # Make provision hang forever and resist cancellation
        provision_entered = asyncio.Event()
        stop_provision = asyncio.Event()

        async def _stuck_provision(*args, **kwargs):
            provision_entered.set()
            while not stop_provision.is_set():
                try:
                    await asyncio.sleep(3600)
                except asyncio.CancelledError:
                    task = asyncio.current_task()
                    assert task is not None
                    task.uncancel()

        mock_execution_env.provision = AsyncMock(side_effect=_stuck_provision)

        # Reduce timeouts for fast test
        original_cancel = store.cancel_environment_task

        async def _fast_cancel(eid: str, *, wait_secs: float = 0.1) -> bool:
            return await original_cancel(eid, wait_secs=wait_secs)

        monkeypatch.setattr(store, "cancel_environment_task", _fast_cancel)
        monkeypatch.setattr(run_module, "_PRIOR_TASK_TIMEOUT", 0.1)

        # Start provision
        prov_resp = await client.post(
            "/api/v1/run/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        assert prov_resp.status_code == 200
        env_id = prov_resp.json()["env_id"]
        await provision_entered.wait()

        # Teardown — should not hang despite stuck prior task
        resp = await client.post(
            f"/api/v1/run/{env_id}/teardown",
            headers=auth_headers,
        )
        assert resp.status_code == 200

        # Teardown should complete within a few seconds (not hang)
        deadline = asyncio.get_event_loop().time() + 3.0
        while asyncio.get_event_loop().time() < deadline:
            await asyncio.sleep(0.05)
            record = await store.get_environment(env_id)
            if record is None:
                break
        assert await store.get_environment(env_id) is None

        # Cleanup: unblock the stuck provision task so it doesn't leak
        stop_provision.set()
        await asyncio.sleep(0.05)
