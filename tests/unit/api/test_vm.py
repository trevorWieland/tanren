"""Tests for VM endpoints."""

from __future__ import annotations

import asyncio
import contextlib
from unittest.mock import AsyncMock

import pytest

from tanren_core.adapters.remote_types import VMAssignment, VMHandle


async def _wait_vm_provisioned(client, env_id, auth_headers, *, max_secs: float = 2.0):
    """Poll until VM provision reaches 'active' status."""
    deadline = asyncio.get_event_loop().time() + max_secs
    while asyncio.get_event_loop().time() < deadline:
        await asyncio.sleep(0.02)
        r = await client.get(f"/api/v1/vm/provision/{env_id}", headers=auth_headers)
        if r.json()["status"] == "active":
            return
    raise AssertionError(f"VM provision {env_id} did not reach 'active' within {max_secs}s")


@pytest.mark.api
class TestVM:
    async def test_list_vms_empty(self, client, auth_headers):
        resp = await client.get("/api/v1/vm", headers=auth_headers)
        assert resp.status_code == 200
        assert resp.json() == []

    async def test_list_vms_with_assignments(self, client, auth_headers, app):
        app.state.vm_state_store.get_active_assignments.return_value = [
            VMAssignment(
                vm_id="vm-1",
                workflow_id="wf-1",
                project="proj",
                spec="specs/a",
                host="10.0.0.1",
                assigned_at="2026-01-01T00:00:00Z",
            ),
        ]

        resp = await client.get("/api/v1/vm", headers=auth_headers)
        assert resp.status_code == 200
        data = resp.json()
        assert len(data) == 1
        assert data[0]["vm_id"] == "vm-1"
        assert data[0]["host"] == "10.0.0.1"
        assert data[0]["status"] == "active"

    async def test_list_vms_no_state_store(self, client, auth_headers, app):
        app.state.vm_state_store = None
        resp = await client.get("/api/v1/vm", headers=auth_headers)
        assert resp.status_code == 200
        assert resp.json() == []

    async def test_release_vm_not_found(self, client, auth_headers):
        resp = await client.delete("/api/v1/vm/nonexistent-vm", headers=auth_headers)
        assert resp.status_code == 404

    async def test_release_vm(self, client, auth_headers, app):
        app.state.vm_state_store.get_assignment.return_value = VMAssignment(
            vm_id="vm-1",
            workflow_id="wf-1",
            project="proj",
            spec="specs/a",
            host="10.0.0.1",
            assigned_at="2026-01-01T00:00:00Z",
        )

        resp = await client.delete("/api/v1/vm/vm-1", headers=auth_headers)
        assert resp.status_code == 200
        data = resp.json()
        assert data["vm_id"] == "vm-1"
        assert data["status"] == "released"

    async def test_provision_vm_returns_accepted(self, client, auth_headers):
        resp = await client.post(
            "/api/v1/vm/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        assert resp.status_code == 200
        data = resp.json()
        assert "env_id" in data
        assert data["status"] == "provisioning"

        # Wait for background to complete and verify via status endpoint
        await _wait_vm_provisioned(client, data["env_id"], auth_headers)
        status = await client.get(f"/api/v1/vm/provision/{data['env_id']}", headers=auth_headers)
        status_data = status.json()
        assert status_data["vm_id"] is not None
        assert status_data["host"] is not None

    async def test_provision_vm_no_execution_env(self, client, auth_headers, app):
        app.state.execution_env = None
        resp = await client.post(
            "/api/v1/vm/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        assert resp.status_code == 500

    async def test_release_vm_calls_provider(self, client, auth_headers, app, mock_execution_env):
        """release_vm calls execution_env.release_vm with a VMHandle."""
        app.state.vm_state_store.get_assignment.return_value = VMAssignment(
            vm_id="vm-1",
            workflow_id="wf-1",
            project="proj",
            spec="specs/a",
            host="10.0.0.1",
            assigned_at="2026-01-01T00:00:00Z",
        )
        mock_execution_env.release_vm = pytest.importorskip("unittest.mock").AsyncMock()

        resp = await client.delete("/api/v1/vm/vm-1", headers=auth_headers)
        assert resp.status_code == 200
        mock_execution_env.release_vm.assert_called_once()
        call_arg = mock_execution_env.release_vm.call_args[0][0]
        assert isinstance(call_arg, VMHandle)
        assert call_arg.vm_id == "vm-1"

    async def test_provision_vm_closes_ssh_connection(
        self, client, auth_headers, mock_execution_env
    ):
        """provision_vm closes the SSH connection in background task."""
        resp = await client.post(
            "/api/v1/vm/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        assert resp.status_code == 200
        env_id = resp.json()["env_id"]

        # Wait for background provision to complete
        await _wait_vm_provisioned(client, env_id, auth_headers)

        handle = mock_execution_env.provision.return_value
        handle.runtime.connection.close.assert_called_once()

    async def test_release_vm_provider_failure_propagates(
        self, client, auth_headers, app, mock_execution_env
    ):
        """Provider release_vm failure returns structured 500; record_release is NOT called."""
        app.state.vm_state_store.get_assignment.return_value = VMAssignment(
            vm_id="vm-1",
            workflow_id="wf-1",
            project="proj",
            spec="specs/a",
            host="10.0.0.1",
            assigned_at="2026-01-01T00:00:00Z",
        )
        mock_execution_env.release_vm = AsyncMock(side_effect=RuntimeError("provider exploded"))

        resp = await client.delete("/api/v1/vm/vm-1", headers=auth_headers)
        assert resp.status_code == 500
        data = resp.json()
        assert data["error_code"] == "service_error"
        app.state.vm_state_store.record_release.assert_not_called()

    async def test_provision_vm_wraps_provider_exception(
        self, client, auth_headers, mock_execution_env
    ):
        """provision_vm surfaces provider failure via status polling."""
        mock_execution_env.provision = AsyncMock(side_effect=RuntimeError("boom"))

        resp = await client.post(
            "/api/v1/vm/provision",
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

    async def test_derive_provider_raises_on_bad_config(self, client, auth_headers, app):
        """_derive_provider wraps config load errors in ServiceError (500)."""
        app.state.config.remote_config_path = "/nonexistent/bad-config.toml"
        app.state.vm_state_store.get_active_assignments.return_value = []

        resp = await client.get("/api/v1/vm", headers=auth_headers)
        assert resp.status_code == 500
        data = resp.json()
        assert data["error_code"] == "service_error"
        assert "Failed to load remote config" in data["detail"]
        assert "/nonexistent/bad-config.toml" not in data["detail"]

    async def test_release_vm_no_execution_env_returns_500(self, client, auth_headers, app):
        """release_vm returns 500 when execution_env is None; record_release is NOT called."""
        from tanren_core.adapters.remote_types import VMAssignment  # noqa: PLC0415

        app.state.execution_env = None
        app.state.vm_state_store.get_assignment.return_value = VMAssignment(
            vm_id="vm-1",
            workflow_id="wf-1",
            project="proj",
            spec="specs/a",
            host="10.0.0.1",
            assigned_at="2026-01-01T00:00:00Z",
        )

        resp = await client.delete("/api/v1/vm/vm-1", headers=auth_headers)
        assert resp.status_code == 500
        data = resp.json()
        assert data["error_code"] == "service_error"
        app.state.vm_state_store.record_release.assert_not_called()

    async def test_dry_run(self, client, auth_headers):
        resp = await client.post(
            "/api/v1/vm/dry-run",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        assert resp.status_code == 200
        data = resp.json()
        assert "requirements" in data

    async def test_provision_cancelled_after_handle_triggers_cleanup(
        self, client, auth_headers, app, mock_execution_env, monkeypatch
    ):
        """VM provision cancelled after handle obtained → finally block calls teardown."""
        from tanren_api.state import _UNSET  # noqa: PLC0415, PLC2701

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
            "/api/v1/vm/provision",
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

    async def test_provision_does_not_orphan_handle_when_record_removed(
        self, client, auth_headers, app, mock_execution_env, monkeypatch
    ):
        """VM provision cleans up handle when environment record is removed mid-provision."""
        store = app.state.api_store
        original_handle = mock_execution_env.provision.return_value

        # Make provision block so we can remove the record while it's in-flight
        provision_entered = asyncio.Event()
        provision_release = asyncio.Event()

        async def _blocking_provision(*args, **kwargs):
            provision_entered.set()
            await provision_release.wait()
            return original_handle

        mock_execution_env.provision = AsyncMock(side_effect=_blocking_provision)

        resp = await client.post(
            "/api/v1/vm/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        assert resp.status_code == 200
        env_id = resp.json()["env_id"]

        # Wait for provision to start
        await provision_entered.wait()

        # Remove the environment record (simulating concurrent teardown)
        removed = await store.remove_environment(env_id)
        assert removed is not None

        # Release provision — try_transition_environment returns None (record gone)
        provision_release.set()
        await asyncio.sleep(0.1)

        # Finally block should have called teardown with the handle (no orphan)
        mock_execution_env.teardown.assert_awaited_once_with(original_handle)

    async def test_provision_sets_completed_at_on_success(self, client, auth_headers, app):
        """Successful provision sets completed_at on the environment record."""
        resp = await client.post(
            "/api/v1/vm/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        assert resp.status_code == 200
        env_id = resp.json()["env_id"]

        await _wait_vm_provisioned(client, env_id, auth_headers)

        record = await app.state.api_store.get_environment(env_id)
        assert record is not None
        assert record.completed_at is not None

    async def test_provision_status_shows_failed_on_failure(
        self, client, auth_headers, mock_execution_env
    ):
        """GET /vm/provision/{env_id} returns 'failed' when provisioning fails."""
        mock_execution_env.provision = AsyncMock(side_effect=RuntimeError("boom"))

        resp = await client.post(
            "/api/v1/vm/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        assert resp.status_code == 200
        env_id = resp.json()["env_id"]

        # Wait for background task to fail
        await asyncio.sleep(0.05)

        status_resp = await client.get(
            f"/api/v1/vm/provision/{env_id}",
            headers=auth_headers,
        )
        assert status_resp.status_code == 200
        assert status_resp.json()["status"] == "failed"
