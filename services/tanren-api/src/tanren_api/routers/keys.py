"""API key management endpoints."""

from __future__ import annotations

from typing import Annotated

from fastapi import APIRouter, Depends, Query
from fastapi import Path as PathParam

from tanren_api.auth import require_scope, resolve_auth
from tanren_api.dependencies import get_auth_store, get_event_store
from tanren_api.models import (
    CreateKeyRequest,
    CreateKeyResponse,
    KeyRevokedResponse,
    KeySummary,
    RotateKeyRequest,
)
from tanren_api.services.keys import KeyService
from tanren_core.store.auth_protocols import AuthStore
from tanren_core.store.auth_views import AuthContext
from tanren_core.store.protocols import EventStore

router = APIRouter(tags=["keys"])


@router.post("/keys")
async def create_key(
    body: CreateKeyRequest,
    auth: Annotated[AuthContext, Depends(require_scope("admin:keys"))],
    event_store: Annotated[EventStore, Depends(get_event_store)],
    auth_store: Annotated[AuthStore, Depends(get_auth_store)],
) -> CreateKeyResponse:
    """Create a new API key.

    Returns:
        CreateKeyResponse with the full key (shown only once).
    """
    service = KeyService(event_store=event_store, auth_store=auth_store)

    resource_limits_json: str | None = None
    if body.resource_limits is not None:
        resource_limits_json = body.resource_limits.model_dump_json()

    raw_key, key_view = await service.create(
        user_id=body.user_id,
        name=body.name,
        scopes=body.scopes,
        resource_limits_json=resource_limits_json,
        expires_at=body.expires_at,
        caller_scopes=auth.scopes,
    )

    return CreateKeyResponse(
        key_id=key_view.key_id,
        key=raw_key,
        key_prefix=key_view.key_prefix,
        name=key_view.name,
        scopes=key_view.scopes,
        created_at=key_view.created_at,
        expires_at=key_view.expires_at,
    )


@router.get("/keys")
async def list_keys(
    auth: Annotated[AuthContext, Depends(resolve_auth)],
    event_store: Annotated[EventStore, Depends(get_event_store)],
    auth_store: Annotated[AuthStore, Depends(get_auth_store)],
    user_id: Annotated[str | None, Query(description="Filter by user ID")] = None,
    limit: Annotated[int, Query(ge=1, le=100, description="Page size")] = 50,
    offset: Annotated[int, Query(ge=0, description="Page offset")] = 0,
) -> list[KeySummary]:
    """List API keys visible to the caller.

    Admins see all keys (optionally filtered by user_id).
    Non-admins see only their own keys.

    Returns:
        List of KeySummary objects (never includes key hashes).
    """
    service = KeyService(event_store=event_store, auth_store=auth_store)

    keys = await service.list(
        user_id=user_id,
        caller_scopes=auth.scopes,
        caller_user_id=auth.user.user_id,
        limit=limit,
        offset=offset,
    )

    return [
        KeySummary(
            key_id=k.key_id,
            user_id=k.user_id,
            name=k.name,
            key_prefix=k.key_prefix,
            scopes=k.scopes,
            created_at=k.created_at,
            expires_at=k.expires_at,
            revoked_at=k.revoked_at,
        )
        for k in keys
    ]


@router.delete("/keys/{key_id}")
async def revoke_key(
    key_id: Annotated[str, PathParam(description="API key ID to revoke")],
    auth: Annotated[AuthContext, Depends(require_scope("admin:keys"))],  # noqa: ARG001 — required for scope enforcement
    event_store: Annotated[EventStore, Depends(get_event_store)],
    auth_store: Annotated[AuthStore, Depends(get_auth_store)],
) -> KeyRevokedResponse:
    """Revoke an API key.

    Returns:
        KeyRevokedResponse confirming the revocation.
    """
    service = KeyService(event_store=event_store, auth_store=auth_store)
    await service.revoke(key_id)
    return KeyRevokedResponse(key_id=key_id)


@router.post("/keys/{key_id}/rotate")
async def rotate_key(
    key_id: Annotated[str, PathParam(description="API key ID to rotate")],
    body: RotateKeyRequest,
    auth: Annotated[AuthContext, Depends(require_scope("admin:keys"))],
    event_store: Annotated[EventStore, Depends(get_event_store)],
    auth_store: Annotated[AuthStore, Depends(get_auth_store)],
) -> CreateKeyResponse:
    """Rotate an API key — create a replacement with a grace period.

    The old key continues to work until the grace period expires.

    Returns:
        CreateKeyResponse with the new full key (shown only once).
    """
    service = KeyService(event_store=event_store, auth_store=auth_store)

    raw_key, key_view = await service.rotate(
        key_id,
        grace_period_hours=body.grace_period_hours,
        caller_scopes=auth.scopes,
    )

    return CreateKeyResponse(
        key_id=key_view.key_id,
        key=raw_key,
        key_prefix=key_view.key_prefix,
        name=key_view.name,
        scopes=key_view.scopes,
        created_at=key_view.created_at,
        expires_at=key_view.expires_at,
    )
