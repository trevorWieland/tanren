"""Read-only view models for auth projection tables."""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict, Field

from tanren_core.store.auth_events import ResourceLimits


class UserView(BaseModel):
    """Read-only projection of a user."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    user_id: str
    name: str
    email: str | None
    role: str
    is_active: bool
    created_at: str
    updated_at: str


class ApiKeyView(BaseModel):
    """Read-only projection of an API key (never includes the raw key)."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    key_id: str
    user_id: str
    name: str
    key_prefix: str
    key_hash: str
    scopes: list[str]
    resource_limits: ResourceLimits | None = None
    created_at: str
    expires_at: str | None = None
    revoked_at: str | None = None
    grace_replaced_by: str | None = None


class AuthContext(BaseModel):
    """Resolved authentication context injected into every request."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    user: UserView = Field(..., description="Resolved user from the API key")
    key: ApiKeyView = Field(..., description="The API key used for this request")
    scopes: frozenset[str] = Field(..., description="Effective permission scopes")
    resource_limits: ResourceLimits | None = Field(
        default=None, description="Resource ceilings from the key"
    )
