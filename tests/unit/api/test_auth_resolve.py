"""Tests for auth resolution logic — key validation, expiry, revocation, scopes."""

from __future__ import annotations

import json

import pytest

from tanren_api.key_utils import generate_api_key

TEST_API_KEY = "tnrn_testpfx1_secretrandompartforunittesting"


@pytest.mark.api
class TestAuthResolveValidKey:
    async def test_valid_key_resolves(self, client, auth_headers) -> None:
        """The seeded admin key should resolve and grant access."""
        resp = await client.get("/api/v1/users", headers=auth_headers)
        assert resp.status_code == 200

    async def test_created_key_resolves(self, client, auth_headers, sqlite_store) -> None:
        """A freshly created key should work for endpoints within its scopes."""
        # Create a user
        create_user_resp = await client.post(
            "/api/v1/users",
            headers=auth_headers,
            json={"name": "AuthTestUser"},
        )
        user_id = create_user_resp.json()["user_id"]

        # Create a scoped key with events:read
        create_key_resp = await client.post(
            "/api/v1/keys",
            headers=auth_headers,
            json={
                "user_id": user_id,
                "name": "scoped-key",
                "scopes": ["events:read"],
            },
        )
        raw_key = create_key_resp.json()["key"]

        # Use the new key to access events:read endpoint
        resp = await client.get(
            "/api/v1/events",
            headers={"X-API-Key": raw_key},
        )
        assert resp.status_code == 200


@pytest.mark.api
class TestAuthResolveInvalidKey:
    async def test_invalid_key_returns_401(self, client) -> None:
        resp = await client.get(
            "/api/v1/users",
            headers={"X-API-Key": "tnrn_fake1234_notarealkey"},
        )
        assert resp.status_code == 401
        assert resp.json()["error_code"] == "authentication_error"

    async def test_missing_header_returns_422(self, client) -> None:
        resp = await client.get("/api/v1/users")
        assert resp.status_code == 422


@pytest.mark.api
class TestAuthResolveRevokedKey:
    async def test_revoked_key_returns_401(self, client, auth_headers) -> None:
        # Create a user and key
        create_user_resp = await client.post(
            "/api/v1/users",
            headers=auth_headers,
            json={"name": "RevokedKeyUser"},
        )
        user_id = create_user_resp.json()["user_id"]

        create_key_resp = await client.post(
            "/api/v1/keys",
            headers=auth_headers,
            json={
                "user_id": user_id,
                "name": "to-revoke",
                "scopes": ["dispatch:read", "config:read"],
            },
        )
        raw_key = create_key_resp.json()["key"]
        key_id = create_key_resp.json()["key_id"]

        # Revoke the key
        await client.delete(f"/api/v1/keys/{key_id}", headers=auth_headers)

        # Try to use the revoked key
        resp = await client.get(
            "/api/v1/config",
            headers={"X-API-Key": raw_key},
        )
        assert resp.status_code == 401


@pytest.mark.api
class TestAuthResolveExpiredKey:
    async def test_expired_key_returns_401(self, client, auth_headers, sqlite_store) -> None:
        # Create a user
        create_user_resp = await client.post(
            "/api/v1/users",
            headers=auth_headers,
            json={"name": "ExpiredKeyUser"},
        )
        user_id = create_user_resp.json()["user_id"]

        # Create a key that is already expired (expires_at in the past)
        raw_key, prefix, key_hash = generate_api_key()
        key_id = "expired-key-id-001"
        await sqlite_store.create_api_key(
            key_id=key_id,
            user_id=user_id,
            name="expired-key",
            key_prefix=prefix,
            key_hash=key_hash,
            scopes_json=json.dumps(["dispatch:read", "config:read"]),
            resource_limits_json=None,
            expires_at="2020-01-01T00:00:00Z",  # Past date
        )

        # Try to use the expired key
        resp = await client.get(
            "/api/v1/config",
            headers={"X-API-Key": raw_key},
        )
        assert resp.status_code == 401


@pytest.mark.api
class TestAuthResolveScopeEnforcement:
    async def test_insufficient_scope_returns_403(self, client, auth_headers) -> None:
        # Create a user and key with only dispatch:read scope
        create_user_resp = await client.post(
            "/api/v1/users",
            headers=auth_headers,
            json={"name": "ScopeTestUser"},
        )
        user_id = create_user_resp.json()["user_id"]

        create_key_resp = await client.post(
            "/api/v1/keys",
            headers=auth_headers,
            json={
                "user_id": user_id,
                "name": "limited-scope-key",
                "scopes": ["dispatch:read"],
            },
        )
        raw_key = create_key_resp.json()["key"]

        # Try to access admin:users endpoint (requires admin:users scope)
        resp = await client.get(
            "/api/v1/users",
            headers={"X-API-Key": raw_key},
        )
        assert resp.status_code == 403
        assert resp.json()["error_code"] == "forbidden"
