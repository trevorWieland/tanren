"""Tests for VM endpoints."""

from __future__ import annotations

from unittest.mock import AsyncMock

import pytest

from tanren_core.adapters.remote_types import VMAssignment, VMHandle


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

    async def test_provision_vm_returns_handle(self, client, auth_headers):
        resp = await client.post(
            "/api/v1/vm/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        assert resp.status_code == 200
        data = resp.json()
        assert "vm_id" in data
        assert "host" in data
        assert "provider" in data

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
        """provision_vm closes the SSH connection to prevent leak."""
        resp = await client.post(
            "/api/v1/vm/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        assert resp.status_code == 200
        handle = mock_execution_env.provision.return_value
        handle.runtime.connection.close.assert_called_once()

    async def test_release_vm_provider_failure_propagates(
        self, client, auth_headers, app, mock_execution_env
    ):
        """Provider release_vm failure propagates; record_release is NOT called."""
        app.state.vm_state_store.get_assignment.return_value = VMAssignment(
            vm_id="vm-1",
            workflow_id="wf-1",
            project="proj",
            spec="specs/a",
            host="10.0.0.1",
            assigned_at="2026-01-01T00:00:00Z",
        )
        mock_execution_env.release_vm = AsyncMock(side_effect=RuntimeError("provider exploded"))

        with pytest.raises(RuntimeError, match="provider exploded"):
            await client.delete("/api/v1/vm/vm-1", headers=auth_headers)
        app.state.vm_state_store.record_release.assert_not_called()

    async def test_derive_provider_raises_on_bad_config(self, client, auth_headers, app):
        """_derive_provider propagates config load errors."""
        app.state.config.remote_config_path = "/nonexistent/bad-config.toml"
        app.state.vm_state_store.get_active_assignments.return_value = []

        with pytest.raises(FileNotFoundError):
            await client.get("/api/v1/vm", headers=auth_headers)

    async def test_dry_run(self, client, auth_headers):
        resp = await client.post(
            "/api/v1/vm/dry-run",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        assert resp.status_code == 200
        data = resp.json()
        assert "requirements" in data
