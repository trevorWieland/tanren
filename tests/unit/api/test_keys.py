"""Tests for API key management endpoints."""

from __future__ import annotations

import pytest

TEST_API_KEY = "tnrn_testpfx1_secretrandompartforunittesting"


async def _create_test_user(client, auth_headers, name="KeyTestUser"):
    """Helper to create a user and return its user_id."""
    resp = await client.post(
        "/api/v1/users",
        headers=auth_headers,
        json={"name": name, "email": f"{name.lower()}@test.com"},
    )
    assert resp.status_code == 200
    return resp.json()["user_id"]


@pytest.mark.api
class TestCreateKey:
    async def test_create_key_returns_raw_key(self, client, auth_headers) -> None:
        user_id = await _create_test_user(client, auth_headers)
        resp = await client.post(
            "/api/v1/keys",
            headers=auth_headers,
            json={
                "user_id": user_id,
                "name": "test-key",
                "scopes": ["dispatch:create", "dispatch:read"],
            },
        )
        assert resp.status_code == 200
        body = resp.json()
        assert body["key"].startswith("tnrn_")
        assert "key_id" in body
        assert body["name"] == "test-key"
        assert body["scopes"] == ["dispatch:create", "dispatch:read"]
        assert "created_at" in body
        assert len(body["key_prefix"]) == 8

    async def test_create_key_with_wildcard_scopes(self, client, auth_headers) -> None:
        user_id = await _create_test_user(client, auth_headers, "WildcardUser")
        resp = await client.post(
            "/api/v1/keys",
            headers=auth_headers,
            json={
                "user_id": user_id,
                "name": "admin-key",
                "scopes": ["*"],
            },
        )
        assert resp.status_code == 200
        assert resp.json()["scopes"] == ["*"]

    async def test_create_key_with_resource_limits(self, client, auth_headers) -> None:
        user_id = await _create_test_user(client, auth_headers, "LimitedUser")
        resp = await client.post(
            "/api/v1/keys",
            headers=auth_headers,
            json={
                "user_id": user_id,
                "name": "limited-key",
                "scopes": ["dispatch:create"],
                "resource_limits": {
                    "max_dispatches_per_hour": 10,
                    "max_concurrent_vms": 2,
                    "max_cost_per_day": 50.0,
                },
            },
        )
        assert resp.status_code == 200

    async def test_create_key_with_expiry(self, client, auth_headers) -> None:
        user_id = await _create_test_user(client, auth_headers, "ExpiryUser")
        resp = await client.post(
            "/api/v1/keys",
            headers=auth_headers,
            json={
                "user_id": user_id,
                "name": "expiring-key",
                "scopes": ["dispatch:read"],
                "expires_at": "2030-12-31T23:59:59Z",
            },
        )
        assert resp.status_code == 200
        assert resp.json()["expires_at"] == "2030-12-31T23:59:59Z"


@pytest.mark.api
class TestListKeys:
    async def test_list_keys_returns_list(self, client, auth_headers) -> None:
        resp = await client.get("/api/v1/keys", headers=auth_headers)
        assert resp.status_code == 200
        keys = resp.json()
        assert isinstance(keys, list)
        # Should include at least the seeded admin key
        assert len(keys) >= 1

    async def test_list_keys_includes_created_key(self, client, auth_headers) -> None:
        user_id = await _create_test_user(client, auth_headers, "ListKeysUser")
        create_resp = await client.post(
            "/api/v1/keys",
            headers=auth_headers,
            json={
                "user_id": user_id,
                "name": "list-test-key",
                "scopes": ["dispatch:read"],
            },
        )
        key_id = create_resp.json()["key_id"]

        resp = await client.get("/api/v1/keys", headers=auth_headers)
        assert resp.status_code == 200
        key_ids = [k["key_id"] for k in resp.json()]
        assert key_id in key_ids

    async def test_list_keys_filter_by_user(self, client, auth_headers) -> None:
        user_id = await _create_test_user(client, auth_headers, "FilterUser")
        await client.post(
            "/api/v1/keys",
            headers=auth_headers,
            json={
                "user_id": user_id,
                "name": "filter-key",
                "scopes": ["dispatch:read"],
            },
        )

        resp = await client.get(f"/api/v1/keys?user_id={user_id}", headers=auth_headers)
        assert resp.status_code == 200
        keys = resp.json()
        for k in keys:
            assert k["user_id"] == user_id


@pytest.mark.api
class TestRevokeKey:
    async def test_revoke_key(self, client, auth_headers) -> None:
        user_id = await _create_test_user(client, auth_headers, "RevokeUser")
        create_resp = await client.post(
            "/api/v1/keys",
            headers=auth_headers,
            json={
                "user_id": user_id,
                "name": "revoke-me",
                "scopes": ["dispatch:read"],
            },
        )
        key_id = create_resp.json()["key_id"]

        resp = await client.delete(f"/api/v1/keys/{key_id}", headers=auth_headers)
        assert resp.status_code == 200
        body = resp.json()
        assert body["key_id"] == key_id
        assert body["status"] == "revoked"


@pytest.mark.api
class TestRotateKey:
    async def test_rotate_key(self, client, auth_headers) -> None:
        user_id = await _create_test_user(client, auth_headers, "RotateUser")
        create_resp = await client.post(
            "/api/v1/keys",
            headers=auth_headers,
            json={
                "user_id": user_id,
                "name": "rotate-me",
                "scopes": ["dispatch:read"],
            },
        )
        old_key_id = create_resp.json()["key_id"]

        resp = await client.post(
            f"/api/v1/keys/{old_key_id}/rotate",
            headers=auth_headers,
            json={"grace_period_hours": 24},
        )
        assert resp.status_code == 200
        body = resp.json()
        assert body["key"].startswith("tnrn_")
        assert body["key_id"] != old_key_id
        assert body["name"] == "rotate-me"
        assert body["scopes"] == ["dispatch:read"]
