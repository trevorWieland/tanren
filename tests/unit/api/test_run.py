"""Tests for run lifecycle endpoints."""

from __future__ import annotations

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
