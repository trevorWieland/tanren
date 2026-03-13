"""Tests for health endpoints."""

import pytest


@pytest.mark.api
class TestHealth:
    async def test_health_returns_200(self, client):
        resp = await client.get("/api/v1/health")
        assert resp.status_code == 200
        data = resp.json()
        assert data["status"] == "ok"
        assert data["version"] == "0.1.0"
        assert "uptime_seconds" in data

    async def test_health_requires_no_auth(self, client):
        # Health endpoint has no auth dependency
        resp = await client.get("/api/v1/health")
        assert resp.status_code == 200

    async def test_readiness_returns_200(self, client):
        resp = await client.get("/api/v1/health/ready")
        assert resp.status_code == 200
        assert resp.json()["status"] == "ready"
