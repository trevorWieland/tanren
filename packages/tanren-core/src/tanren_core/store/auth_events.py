"""Auth lifecycle events for users and API keys.

These events are appended to the shared ``events`` table alongside dispatch
events.  Each carries ``entity_type`` set to ``USER`` or ``API_KEY`` so they
can be queried independently.
"""

from __future__ import annotations

from typing import Literal

from pydantic import BaseModel, ConfigDict, Field

from tanren_core.adapters.events import Event

# ── Shared models ────────────────────────────────────────────────────────


class ResourceLimits(BaseModel):
    """Per-key resource ceilings (all nullable = unlimited)."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    max_concurrent_vms: int | None = Field(
        default=None, ge=1, description="Max VMs this key may have active at once"
    )
    max_dispatches_per_hour: int | None = Field(
        default=None, ge=1, description="Max dispatches allowed in a sliding 60-minute window"
    )
    max_cost_per_day: float | None = Field(
        default=None, ge=0.0, description="Max USD spend in a calendar day"
    )


# ── User events ──────────────────────────────────────────────────────────


class UserCreated(Event):
    """A new user was created."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    type: Literal["user_created"] = Field(
        default="user_created", description="Event type discriminator"
    )
    entity_type: str = Field(default="user")
    user_id: str = Field(..., description="UUID of the created user")
    name: str = Field(..., description="Display name")
    email: str | None = Field(default=None, description="Email address (optional)")
    role: str = Field(default="member", description="Convenience role label")


class UserUpdated(Event):
    """A user's profile was modified."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    type: Literal["user_updated"] = Field(
        default="user_updated", description="Event type discriminator"
    )
    entity_type: str = Field(default="user")
    name: str | None = Field(default=None, description="New display name")
    email: str | None = Field(default=None, description="New email address")
    role: str | None = Field(default=None, description="New role label")


class UserDeactivated(Event):
    """A user was deactivated (soft-delete)."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    type: Literal["user_deactivated"] = Field(
        default="user_deactivated", description="Event type discriminator"
    )
    entity_type: str = Field(default="user")


# ── API key events ───────────────────────────────────────────────────────


class KeyCreated(Event):
    """A new API key was issued."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    type: Literal["key_created"] = Field(
        default="key_created", description="Event type discriminator"
    )
    entity_type: str = Field(default="api_key")
    key_id: str = Field(..., description="UUID of the key")
    user_id: str = Field(..., description="Owning user UUID")
    name: str = Field(..., description="Human-readable key name")
    key_prefix: str = Field(..., description="8-char plaintext prefix for identification")
    scopes: list[str] = Field(..., description="Granted scope strings")
    resource_limits: ResourceLimits | None = Field(default=None, description="Resource ceilings")
    expires_at: str | None = Field(default=None, description="ISO 8601 expiry (optional)")


class KeyRevoked(Event):
    """An API key was revoked."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    type: Literal["key_revoked"] = Field(
        default="key_revoked", description="Event type discriminator"
    )
    entity_type: str = Field(default="api_key")


class KeyRotated(Event):
    """An API key was rotated — a replacement key was created."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    type: Literal["key_rotated"] = Field(
        default="key_rotated", description="Event type discriminator"
    )
    entity_type: str = Field(default="api_key")
    new_key_id: str = Field(..., description="UUID of the replacement key")
    grace_expires_at: str | None = Field(
        default=None, description="ISO 8601 timestamp after which the old key stops working"
    )
