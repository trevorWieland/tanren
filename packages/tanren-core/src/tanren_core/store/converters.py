"""Converters between SQLAlchemy ORM rows and domain view/model types.

Each function is a pure transformation with fully typed signatures so
that ty/Pyright catches mismatches between ORM ``Mapped[T]`` types and
domain model constructors at build time.
"""

from __future__ import annotations

import json

from tanren_core.schemas import Dispatch, Outcome
from tanren_core.store.auth_events import ResourceLimits
from tanren_core.store.auth_views import ApiKeyView, UserView
from tanren_core.store.enums import DispatchMode, DispatchStatus, Lane, StepStatus, StepType
from tanren_core.store.models import (
    ApiKeyProjection,
    DispatchProjection,
    EventModel,
    StepProjection,
    UserProjection,
)
from tanren_core.store.views import DispatchView, EventRow, QueuedStep, StepView

# ---------------------------------------------------------------------------
# Dispatch
# ---------------------------------------------------------------------------


def dispatch_to_view(row: DispatchProjection) -> DispatchView:
    """Convert an ORM DispatchProjection row to a domain DispatchView."""
    return DispatchView(
        dispatch_id=row.dispatch_id,
        mode=DispatchMode(row.mode),
        status=DispatchStatus(row.status),
        outcome=Outcome(row.outcome) if row.outcome else None,
        lane=Lane(row.lane),
        preserve_on_failure=row.preserve_on_failure,
        dispatch=Dispatch.model_validate(row.dispatch_json),
        user_id=row.user_id or "",
        created_at=row.created_at,
        updated_at=row.updated_at,
    )


# ---------------------------------------------------------------------------
# Step
# ---------------------------------------------------------------------------


def step_to_view(row: StepProjection) -> StepView:
    """Convert an ORM StepProjection row to a domain StepView.

    The domain contract uses ``str | None`` for ``result_json``, so we
    serialize the dict back to a JSON string.
    """
    result_str: str | None = None
    if row.result_json is not None:
        result_str = (
            json.dumps(row.result_json)
            if isinstance(row.result_json, dict)
            else str(row.result_json)
        )

    return StepView(
        step_id=row.step_id,
        dispatch_id=row.dispatch_id,
        step_type=StepType(row.step_type),
        step_sequence=row.step_sequence,
        lane=Lane(row.lane) if row.lane else None,
        status=StepStatus(row.status),
        worker_id=row.worker_id,
        result_json=result_str,
        error=row.error,
        retry_count=row.retry_count,
        created_at=row.created_at,
        updated_at=row.updated_at,
    )


def step_to_queued(row: StepProjection) -> QueuedStep:
    """Convert an ORM StepProjection row to a domain QueuedStep.

    The domain contract uses ``str`` for ``payload_json``, so we
    serialize the dict to a JSON string.
    """
    payload_str = (
        json.dumps(row.payload_json)
        if isinstance(row.payload_json, dict)
        else str(row.payload_json)
    )

    return QueuedStep(
        step_id=row.step_id,
        dispatch_id=row.dispatch_id,
        step_type=StepType(row.step_type),
        step_sequence=row.step_sequence,
        lane=Lane(row.lane) if row.lane else None,
        payload_json=payload_str,
    )


# ---------------------------------------------------------------------------
# Event
# ---------------------------------------------------------------------------


def event_to_record(row: EventModel) -> EventRow:
    """Convert an ORM EventModel row to a domain EventRow."""
    return EventRow(
        id=row.id,
        timestamp=row.timestamp,
        entity_id=row.entity_id,
        entity_type=row.entity_type,
        event_type=row.event_type,
        payload=row.payload,
    )


# ---------------------------------------------------------------------------
# User
# ---------------------------------------------------------------------------


def user_to_view(row: UserProjection) -> UserView:
    """Convert an ORM UserProjection row to a domain UserView."""
    return UserView(
        user_id=row.user_id,
        name=row.name,
        email=row.email,
        role=row.role,
        is_active=row.is_active,
        created_at=row.created_at,
        updated_at=row.updated_at,
    )


# ---------------------------------------------------------------------------
# API Key
# ---------------------------------------------------------------------------


def api_key_to_view(row: ApiKeyProjection) -> ApiKeyView:
    """Convert an ORM ApiKeyProjection row to a domain ApiKeyView.

    Scopes are stored as a JSON list; resource_limits as a JSON dict
    that maps to ``ResourceLimits``.
    """
    scopes: list[str] = list(row.scopes) if row.scopes else []

    resource_limits: ResourceLimits | None = None
    if row.resource_limits:
        resource_limits = ResourceLimits.model_validate(row.resource_limits)

    return ApiKeyView(
        key_id=row.key_id,
        user_id=row.user_id,
        name=row.name,
        key_prefix=row.key_prefix,
        key_hash=row.key_hash,
        scopes=scopes,
        resource_limits=resource_limits,
        created_at=row.created_at,
        expires_at=row.expires_at,
        revoked_at=row.revoked_at,
        grace_replaced_by=row.grace_replaced_by,
    )
