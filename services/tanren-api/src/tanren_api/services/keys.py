"""API key management service — event-sourced with auth_store projections."""

from __future__ import annotations

import builtins
import json
import logging
import uuid
from datetime import UTC, datetime, timedelta

from tanren_api.errors import ForbiddenError, NotFoundError
from tanren_api.key_utils import generate_api_key
from tanren_api.scopes import has_scope, validate_scopes
from tanren_core.store.auth_events import KeyCreated, KeyRevoked, KeyRotated, ResourceLimits
from tanren_core.store.auth_protocols import AuthStore
from tanren_core.store.auth_views import ApiKeyView
from tanren_core.store.protocols import EventStore

logger = logging.getLogger(__name__)


def _now() -> str:
    """Return an ISO 8601 UTC timestamp."""
    return datetime.now(UTC).isoformat().replace("+00:00", "Z")


class KeyService:
    """Stateless API key management — appends events and updates projections."""

    def __init__(self, *, event_store: EventStore, auth_store: AuthStore) -> None:
        """Initialize with store dependencies."""
        self._event_store = event_store
        self._auth_store = auth_store

    async def create(
        self,
        *,
        user_id: str,
        name: str,
        scopes: builtins.list[str],
        resource_limits_json: str | None,
        expires_at: str | None,
        caller_scopes: frozenset[str],
    ) -> tuple[str, ApiKeyView]:
        """Create a new API key.

        Validates scopes, verifies the caller can grant them, generates the
        key, appends a KeyCreated event, and writes the auth_store projection.

        Returns:
            Tuple of ``(raw_key, key_view)`` — the raw key is shown only once.

        Raises:
            ForbiddenError: If the caller lacks a requested scope.
        """
        try:
            validate_scopes(scopes)
        except ValueError as e:
            from tanren_api.errors import ValidationError

            raise ValidationError(str(e)) from e

        # Verify target user exists
        user = await self._auth_store.get_user(user_id)
        if user is None:
            raise NotFoundError(f"User {user_id} not found")

        # Verify caller has all requested scopes
        for scope in scopes:
            if not has_scope(caller_scopes, scope):
                raise ForbiddenError(f"Cannot grant scope you do not have: {scope}")

        key_id = uuid.uuid4().hex
        raw_key, prefix, key_hash = generate_api_key()
        now = _now()

        # Parse resource limits for the event
        resource_limits: ResourceLimits | None = None
        if resource_limits_json is not None:
            resource_limits = ResourceLimits.model_validate_json(resource_limits_json)

        event = KeyCreated(
            timestamp=now,
            entity_id=key_id,
            entity_type="api_key",
            key_id=key_id,
            user_id=user_id,
            name=name,
            key_prefix=prefix,
            scopes=scopes,
            resource_limits=resource_limits,
            expires_at=expires_at,
        )
        await self._event_store.append(event)

        await self._auth_store.create_api_key(
            key_id=key_id,
            user_id=user_id,
            name=name,
            key_prefix=prefix,
            key_hash=key_hash,
            scopes_json=json.dumps(scopes),
            resource_limits_json=resource_limits_json,
            expires_at=expires_at,
        )

        key_view = ApiKeyView(
            key_id=key_id,
            user_id=user_id,
            name=name,
            key_prefix=prefix,
            key_hash=key_hash,
            scopes=scopes,
            resource_limits=resource_limits,
            created_at=now,
            expires_at=expires_at,
        )

        return raw_key, key_view

    async def list(
        self,
        *,
        user_id: str | None,
        caller_scopes: frozenset[str],
        caller_user_id: str,
        limit: int,
        offset: int,
    ) -> builtins.list[ApiKeyView]:
        """List API keys with scope-based filtering.

        If the caller has ``admin:keys`` scope, show all keys (optionally
        filtered by user_id). Otherwise, show only the caller's own keys.

        Returns:
            A list of ApiKeyView objects.
        """
        if has_scope(caller_scopes, "admin:keys"):
            return await self._auth_store.list_api_keys(
                user_id=user_id,
                limit=limit,
                offset=offset,
            )
        # Non-admin callers can only see their own keys
        return await self._auth_store.list_api_keys(
            user_id=caller_user_id,
            limit=limit,
            offset=offset,
        )

    async def revoke(self, key_id: str) -> None:
        """Revoke an API key.

        Appends a KeyRevoked event and updates the auth_store projection.

        Raises:
            NotFoundError: If the key does not exist.
        """
        key = await self._auth_store.get_api_key(key_id)
        if key is None:
            raise NotFoundError(f"API key {key_id} not found")

        event = KeyRevoked(
            timestamp=_now(),
            entity_id=key_id,
            entity_type="api_key",
        )
        await self._event_store.append(event)

        await self._auth_store.revoke_api_key(key_id)

    async def rotate(
        self,
        key_id: str,
        *,
        grace_period_hours: int,
        caller_scopes: frozenset[str],
    ) -> tuple[str, ApiKeyView]:
        """Rotate an API key — create a replacement and grace-expire the old one.

        The old key continues to work until ``now + grace_period_hours``, then
        is treated as revoked. The new key is immediately active.

        Returns:
            Tuple of ``(new_raw_key, new_key_view)``.

        Raises:
            NotFoundError: If the old key does not exist.
        """
        old_key = await self._auth_store.get_api_key(key_id)
        if old_key is None:
            raise NotFoundError(f"API key {key_id} not found")

        # Serialize resource_limits for the create call
        resource_limits_json: str | None = None
        if old_key.resource_limits is not None:
            resource_limits_json = old_key.resource_limits.model_dump_json()

        # Create the replacement key with same user, scopes, resource_limits
        new_raw_key, new_key_view = await self.create(
            user_id=old_key.user_id,
            name=old_key.name,
            scopes=old_key.scopes,
            resource_limits_json=resource_limits_json,
            expires_at=old_key.expires_at,
            caller_scopes=caller_scopes,
        )

        # Grace-expire the old key
        grace_expires = (
            (datetime.now(UTC) + timedelta(hours=grace_period_hours))
            .isoformat()
            .replace("+00:00", "Z")
        )

        await self._auth_store.set_grace_replacement(
            key_id,
            replaced_by=new_key_view.key_id,
            revoked_at=grace_expires,
        )

        # Append rotation event on the old key
        event = KeyRotated(
            timestamp=_now(),
            entity_id=key_id,
            entity_type="api_key",
            new_key_id=new_key_view.key_id,
            grace_expires_at=grace_expires,
        )
        await self._event_store.append(event)

        return new_raw_key, new_key_view
