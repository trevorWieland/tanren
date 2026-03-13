"""Tests for run lifecycle endpoints."""

from __future__ import annotations

import pytest


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
