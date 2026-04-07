"""User management endpoints — CRUD for event-sourced users."""

from __future__ import annotations

from typing import Annotated

from fastapi import APIRouter, Depends, Query
from fastapi import Path as PathParam

from tanren_api.auth import require_scope
from tanren_api.dependencies import get_auth_store, get_event_store
from tanren_api.models import CreateUserRequest, UserDeactivatedResponse, UserResponse
from tanren_api.services.users import UserService
from tanren_core.store.auth_protocols import AuthStore
from tanren_core.store.auth_views import AuthContext
from tanren_core.store.protocols import EventStore

router = APIRouter(tags=["users"])


@router.post("/users")
async def create_user(
    body: CreateUserRequest,
    _auth: Annotated[AuthContext, Depends(require_scope("admin:users"))],
    event_store: Annotated[EventStore, Depends(get_event_store)],
    auth_store: Annotated[AuthStore, Depends(get_auth_store)],
) -> UserResponse:
    """Create a new user.

    Returns:
        UserResponse: The newly created user.
    """
    service = UserService(event_store=event_store, auth_store=auth_store)
    user = await service.create(name=body.name, email=body.email, role=body.role)
    return UserResponse(
        user_id=user.user_id,
        name=user.name,
        email=user.email,
        role=user.role,
        is_active=user.is_active,
        created_at=user.created_at,
    )


@router.get("/users")
async def list_users(
    _auth: Annotated[AuthContext, Depends(require_scope("admin:users"))],
    event_store: Annotated[EventStore, Depends(get_event_store)],
    auth_store: Annotated[AuthStore, Depends(get_auth_store)],
    limit: Annotated[int, Query(ge=1, le=200, description="Page size")] = 50,
    offset: Annotated[int, Query(ge=0, description="Pagination offset")] = 0,
) -> list[UserResponse]:
    """List users with pagination.

    Returns:
        List of UserResponse objects.
    """
    service = UserService(event_store=event_store, auth_store=auth_store)
    users = await service.list(limit=limit, offset=offset)
    return [
        UserResponse(
            user_id=u.user_id,
            name=u.name,
            email=u.email,
            role=u.role,
            is_active=u.is_active,
            created_at=u.created_at,
        )
        for u in users
    ]


@router.get("/users/{user_id}")
async def get_user(
    user_id: Annotated[str, PathParam(description="User identifier")],
    _auth: Annotated[AuthContext, Depends(require_scope("admin:users"))],
    event_store: Annotated[EventStore, Depends(get_event_store)],
    auth_store: Annotated[AuthStore, Depends(get_auth_store)],
) -> UserResponse:
    """Get a single user by ID.

    Returns:
        UserResponse: The requested user.
    """
    service = UserService(event_store=event_store, auth_store=auth_store)
    user = await service.get(user_id)
    return UserResponse(
        user_id=user.user_id,
        name=user.name,
        email=user.email,
        role=user.role,
        is_active=user.is_active,
        created_at=user.created_at,
    )


@router.delete("/users/{user_id}")
async def deactivate_user(
    user_id: Annotated[str, PathParam(description="User identifier")],
    _auth: Annotated[AuthContext, Depends(require_scope("admin:users"))],
    event_store: Annotated[EventStore, Depends(get_event_store)],
    auth_store: Annotated[AuthStore, Depends(get_auth_store)],
) -> UserDeactivatedResponse:
    """Deactivate a user (soft-delete).

    Returns:
        UserDeactivatedResponse: Confirmation of deactivation.
    """
    service = UserService(event_store=event_store, auth_store=auth_store)
    await service.deactivate(user_id)
    return UserDeactivatedResponse(user_id=user_id)
