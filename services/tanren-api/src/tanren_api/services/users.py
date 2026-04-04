"""User management service — event-sourced with auth_store projections."""

from __future__ import annotations

import builtins
import logging
import uuid

from tanren_api.errors import NotFoundError
from tanren_core.store.auth_events import UserCreated, UserDeactivated
from tanren_core.store.auth_protocols import AuthStore
from tanren_core.store.auth_views import UserView
from tanren_core.store.protocols import EventStore
from tanren_core.timestamps import utc_now_iso

logger = logging.getLogger(__name__)


class UserService:
    """Stateless user management — appends events and updates projections."""

    def __init__(self, *, event_store: EventStore, auth_store: AuthStore) -> None:
        """Initialize with store dependencies."""
        self._event_store = event_store
        self._auth_store = auth_store

    async def create(self, *, name: str, email: str | None, role: str) -> UserView:
        """Create a new user.

        Appends a UserCreated event and writes the auth_store projection.

        Returns:
            The newly created UserView.
        """
        user_id = uuid.uuid4().hex
        now = utc_now_iso()

        event = UserCreated(
            timestamp=now,
            entity_id=user_id,
            entity_type="user",
            user_id=user_id,
            name=name,
            email=email,
            role=role,
        )
        await self._event_store.append(event)

        await self._auth_store.create_user(
            user_id=user_id,
            name=name,
            email=email,
            role=role,
        )

        return UserView(
            user_id=user_id,
            name=name,
            email=email,
            role=role,
            is_active=True,
            created_at=now,
            updated_at=now,
        )

    async def get(self, user_id: str) -> UserView:
        """Look up a user by ID.

        Returns:
            The UserView for the given ID.

        Raises:
            NotFoundError: If the user does not exist.
        """
        user = await self._auth_store.get_user(user_id)
        if user is None:
            raise NotFoundError(f"User {user_id} not found")
        return user

    async def list(self, *, limit: int = 50, offset: int = 0) -> builtins.list[UserView]:
        """List users with pagination.

        Returns:
            A list of UserView objects.
        """
        return await self._auth_store.list_users(limit=limit, offset=offset)

    async def deactivate(self, user_id: str) -> None:
        """Deactivate a user (soft-delete).

        Appends a UserDeactivated event and updates the auth_store projection.

        Raises:
            NotFoundError: If the user does not exist.
        """
        user = await self._auth_store.get_user(user_id)
        if user is None:
            raise NotFoundError(f"User {user_id} not found")

        event = UserDeactivated(
            timestamp=utc_now_iso(),
            entity_id=user_id,
            entity_type="user",
        )
        await self._event_store.append(event)

        await self._auth_store.deactivate_user(user_id)
