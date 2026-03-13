"""Tests for dispatch endpoints."""

import pytest


@pytest.mark.api
class TestDispatch:
    async def test_create_dispatch_returns_501(self, client, auth_headers):
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
        assert resp.status_code == 501

    async def test_get_dispatch_returns_501(self, client, auth_headers):
        resp = await client.get("/api/v1/dispatch/some-id", headers=auth_headers)
        assert resp.status_code == 501

    async def test_cancel_dispatch_returns_501(self, client, auth_headers):
        resp = await client.delete("/api/v1/dispatch/some-id", headers=auth_headers)
        assert resp.status_code == 501
