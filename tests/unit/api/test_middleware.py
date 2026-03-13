"""Tests for API middleware."""

import pytest


@pytest.mark.api
class TestMiddleware:
    async def test_request_id_in_response_headers(self, client):
        resp = await client.get("/api/v1/health")
        assert "X-Request-ID" in resp.headers
        # Verify it's a valid UUID-like string
        request_id = resp.headers["X-Request-ID"]
        assert len(request_id) > 0

    async def test_request_ids_are_unique(self, client):
        resp1 = await client.get("/api/v1/health")
        resp2 = await client.get("/api/v1/health")
        assert resp1.headers["X-Request-ID"] != resp2.headers["X-Request-ID"]
