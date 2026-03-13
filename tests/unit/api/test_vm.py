"""Tests for VM endpoints."""

import pytest


@pytest.mark.api
class TestVM:
    async def test_list_vms_returns_501(self, client, auth_headers):
        resp = await client.get("/api/v1/vm", headers=auth_headers)
        assert resp.status_code == 501

    async def test_provision_vm_returns_501(self, client, auth_headers):
        resp = await client.post(
            "/api/v1/vm/provision",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        assert resp.status_code == 501

    async def test_release_vm_returns_501(self, client, auth_headers):
        resp = await client.delete("/api/v1/vm/some-vm-id", headers=auth_headers)
        assert resp.status_code == 501

    async def test_dry_run_returns_501(self, client, auth_headers):
        resp = await client.post(
            "/api/v1/vm/dry-run",
            json={"project": "test", "branch": "main"},
            headers=auth_headers,
        )
        assert resp.status_code == 501
