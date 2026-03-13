"""Tests for dispatch endpoints."""

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

    async def test_cancel_dispatch_not_found(self, client, auth_headers):
        resp = await client.delete("/api/v1/dispatch/nonexistent-id", headers=auth_headers)
        assert resp.status_code == 404
