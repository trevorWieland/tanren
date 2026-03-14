"""Tests for VM endpoints."""

from __future__ import annotations

import asyncio
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
