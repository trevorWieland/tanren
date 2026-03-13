"""Tests for API key authentication."""

import pytest


@pytest.mark.api
class TestAuth:
    async def test_missing_api_key_returns_422(self, client):
        resp = await client.get("/api/v1/config")
        assert resp.status_code == 422

    async def test_wrong_api_key_returns_401(self, client):
        resp = await client.get("/api/v1/config", headers={"X-API-Key": "wrong-key"})
        assert resp.status_code == 401

    async def test_correct_api_key_succeeds(self, client, auth_headers):
        resp = await client.get("/api/v1/config", headers=auth_headers)
        # Config endpoint may return 500 if no WM_* config, but not 401/422
        assert resp.status_code not in (401, 422)
