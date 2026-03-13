"""Tests for VM endpoints."""

from __future__ import annotations

import pytest

from tanren_core.adapters.remote_types import VMAssignment


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

    async def test_dry_run(self, client, auth_headers):
        resp = await client.post(
            "/api/v1/vm/dry-run",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        assert resp.status_code == 200
        data = resp.json()
        assert "requirements" in data
