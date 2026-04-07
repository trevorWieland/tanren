"""Protocol for auth-related store operations (users and API keys)."""

from __future__ import annotations

from typing import Protocol, runtime_checkable

from tanren_core.store.auth_views import ApiKeyView, UserView


@runtime_checkable
class AuthStore(Protocol):
    """Read/write access to user and API key projection tables."""

    # ── Users ────────────────────────────────────────────────────────────

    async def create_user(
        self,
        *,
        user_id: str,
        name: str,
        email: str | None,
        role: str,
    ) -> None:
        """Insert a new user projection row."""
        ...

    async def get_user(self, user_id: str) -> UserView | None:
        """Look up a user by ID."""
        ...

    async def list_users(self, *, limit: int = 50, offset: int = 0) -> list[UserView]:
        """List users with pagination."""
        ...

    async def update_user(
        self,
        user_id: str,
        *,
        name: str | None = None,
        email: str | None = None,
        role: str | None = None,
    ) -> None:
        """Update mutable user fields."""
        ...

    async def deactivate_user(self, user_id: str) -> None:
        """Set ``is_active = false`` on a user."""
        ...

    # ── API keys ─────────────────────────────────────────────────────────

    async def create_api_key(
        self,
        *,
        key_id: str,
        user_id: str,
        name: str,
        key_prefix: str,
        key_hash: str,
        scopes_json: str,
        resource_limits_json: str | None,
        expires_at: str | None,
    ) -> None:
        """Insert a new API key projection row."""
        ...

    async def get_api_key_by_hash(self, key_hash: str) -> ApiKeyView | None:
        """Look up an API key by its SHA-256 hash."""
        ...

    async def get_api_key(self, key_id: str) -> ApiKeyView | None:
        """Look up an API key by ID."""
        ...

    async def list_api_keys(
        self,
        *,
        user_id: str | None = None,
        include_revoked: bool = False,
        limit: int = 50,
        offset: int = 0,
    ) -> list[ApiKeyView]:
        """List API keys, optionally filtered by user."""
        ...

    async def revoke_api_key(self, key_id: str) -> None:
        """Set ``revoked_at`` to now on an API key."""
        ...

    async def set_grace_replacement(
        self, key_id: str, *, replaced_by: str, revoked_at: str
    ) -> None:
        """Mark an old key as replaced during rotation.

        Sets ``grace_replaced_by`` and ``revoked_at`` on the old key.
        """
        ...

    # ── Resource limit queries ───────────────────────────────────────────

    async def count_dispatches_since(self, user_id: str, since: str) -> int:
        """Count dispatches created by *user_id* since *since* (ISO 8601)."""
        ...

    async def count_active_vms(self, user_id: str) -> int:
        """Count VMs currently active for *user_id*."""
        ...

    async def sum_cost_since(self, user_id: str, since: str) -> float:
        """Sum USD cost from TokenUsageRecorded events for *user_id* since *since*."""
        ...

    async def close(self) -> None:
        """Close any resources."""
        ...
