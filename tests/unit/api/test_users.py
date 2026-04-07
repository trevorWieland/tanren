"""Tests for user CRUD endpoints."""

from __future__ import annotations

import pytest

TEST_API_KEY = "tnrn_testpfx1_secretrandompartforunittesting"


@pytest.mark.api
class TestCreateUser:
    async def test_create_user_returns_user(self, client, auth_headers) -> None:
        resp = await client.post(
            "/api/v1/users",
            headers=auth_headers,
            json={"name": "Alice", "email": "alice@example.com", "role": "member"},
        )
        assert resp.status_code == 200
        body = resp.json()
        assert body["name"] == "Alice"
        assert body["email"] == "alice@example.com"
        assert body["role"] == "member"
        assert body["is_active"] is True
        assert "user_id" in body
        assert "created_at" in body

    async def test_create_user_without_email(self, client, auth_headers) -> None:
        resp = await client.post(
            "/api/v1/users",
            headers=auth_headers,
            json={"name": "Bob"},
        )
        assert resp.status_code == 200
        body = resp.json()
        assert body["name"] == "Bob"
        assert body["email"] is None
        assert body["role"] == "member"


@pytest.mark.api
class TestListUsers:
    async def test_list_users_includes_seeded_admin(self, client, auth_headers) -> None:
        resp = await client.get("/api/v1/users", headers=auth_headers)
        assert resp.status_code == 200
        users = resp.json()
        assert isinstance(users, list)
        assert len(users) >= 1
        admin_ids = [u["user_id"] for u in users]
        assert "admin-00000000" in admin_ids

    async def test_list_users_includes_created_user(self, client, auth_headers) -> None:
        # Create a user first
        await client.post(
            "/api/v1/users",
            headers=auth_headers,
            json={"name": "Charlie", "email": "charlie@example.com"},
        )
        resp = await client.get("/api/v1/users", headers=auth_headers)
        assert resp.status_code == 200
        names = [u["name"] for u in resp.json()]
        assert "Charlie" in names


@pytest.mark.api
class TestGetUser:
    async def test_get_existing_user(self, client, auth_headers) -> None:
        # Get the seeded admin user
        resp = await client.get("/api/v1/users/admin-00000000", headers=auth_headers)
        assert resp.status_code == 200
        body = resp.json()
        assert body["user_id"] == "admin-00000000"
        assert body["is_active"] is True

    async def test_get_created_user(self, client, auth_headers) -> None:
        create_resp = await client.post(
            "/api/v1/users",
            headers=auth_headers,
            json={"name": "Diana", "email": "diana@test.com", "role": "admin"},
        )
        user_id = create_resp.json()["user_id"]

        resp = await client.get(f"/api/v1/users/{user_id}", headers=auth_headers)
        assert resp.status_code == 200
        body = resp.json()
        assert body["user_id"] == user_id
        assert body["name"] == "Diana"
        assert body["role"] == "admin"

    async def test_get_nonexistent_user_returns_404(self, client, auth_headers) -> None:
        resp = await client.get("/api/v1/users/nonexistent-user-id", headers=auth_headers)
        assert resp.status_code == 404


@pytest.mark.api
class TestDeactivateUser:
    async def test_deactivate_user(self, client, auth_headers) -> None:
        # Create a user to deactivate
        create_resp = await client.post(
            "/api/v1/users",
            headers=auth_headers,
            json={"name": "Eve"},
        )
        user_id = create_resp.json()["user_id"]

        resp = await client.delete(f"/api/v1/users/{user_id}", headers=auth_headers)
        assert resp.status_code == 200
        body = resp.json()
        assert body["user_id"] == user_id
        assert body["status"] == "deactivated"

        # Verify user is now inactive
        get_resp = await client.get(f"/api/v1/users/{user_id}", headers=auth_headers)
        assert get_resp.status_code == 200
        assert get_resp.json()["is_active"] is False

    async def test_deactivate_nonexistent_user_returns_404(self, client, auth_headers) -> None:
        resp = await client.delete("/api/v1/users/nonexistent-user-id", headers=auth_headers)
        assert resp.status_code == 404
