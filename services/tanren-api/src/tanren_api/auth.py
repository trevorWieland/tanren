"""API authentication — scoped API key resolution and scope enforcement."""

from __future__ import annotations

from collections.abc import Callable, Coroutine
from datetime import UTC, datetime
from typing import Annotated, Any

from fastapi import Depends, Header, Request

from tanren_api.errors import AuthenticationError, ForbiddenError
from tanren_api.key_utils import hash_api_key
from tanren_api.scopes import has_scope
from tanren_core.store.auth_protocols import AuthStore
from tanren_core.store.auth_views import AuthContext


def _utcnow() -> datetime:
    return datetime.now(UTC)


def _parse_iso(ts: str) -> datetime:
    """Parse an ISO 8601 timestamp to an aware datetime."""
    return datetime.fromisoformat(ts)


async def resolve_auth(
    request: Request,
    x_api_key: Annotated[str, Header()],
) -> AuthContext:
    """FastAPI dependency: hash key -> look up -> resolve User + scopes.

    Returns:
        Resolved AuthContext for the request.

    Raises:
        AuthenticationError: If key is invalid, revoked, expired, or user deactivated.
    """
    auth_store: AuthStore = request.app.state.auth_store

    key_hash = hash_api_key(x_api_key)
    key_view = await auth_store.get_api_key_by_hash(key_hash)

    if key_view is None:
        raise AuthenticationError("Invalid API key")

    now = _utcnow()

    # Check revocation (respecting grace period)
    if key_view.revoked_at is not None and _parse_iso(key_view.revoked_at) <= now:
        raise AuthenticationError("API key has been revoked")

    # Check expiry
    if key_view.expires_at is not None and _parse_iso(key_view.expires_at) <= now:
        raise AuthenticationError("API key has expired")

    # Resolve user
    user = await auth_store.get_user(key_view.user_id)
    if user is None or not user.is_active:
        raise AuthenticationError("User not found or deactivated")

    return AuthContext(
        user=user,
        key=key_view,
        scopes=frozenset(key_view.scopes),
        resource_limits=key_view.resource_limits,
    )


def require_scope(
    scope: str,
) -> Callable[..., Coroutine[Any, Any, AuthContext]]:
    """FastAPI dependency factory: enforce a scope on an endpoint.

    Usage::

        @router.post("/dispatch")
        async def create_dispatch(
            auth: Annotated[AuthContext, Depends(require_scope("dispatch:create"))],
            ...
        ) -> ...:

    Returns:
        A dependency that resolves auth and checks the required scope.
    """

    async def _check(  # noqa: RUF029 — FastAPI resolves Depends() at runtime
        auth: Annotated[AuthContext, Depends(resolve_auth)],
    ) -> AuthContext:
        if not has_scope(auth.scopes, scope):
            raise ForbiddenError(f"Missing required scope: {scope}")
        return auth

    return _check
