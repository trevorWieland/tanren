"""Read-only view dataclasses for projection queries."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import TYPE_CHECKING

from tanren_core.schemas import Dispatch, Outcome
from tanren_core.store.enums import DispatchMode, DispatchStatus, Lane, StepStatus, StepType

if TYPE_CHECKING:
    from pydantic import JsonValue


@dataclass(frozen=True)
class DispatchView:
    """Read-only projection of a dispatch."""

    dispatch_id: str
    mode: DispatchMode
    status: DispatchStatus
    outcome: Outcome | None
    lane: Lane
    preserve_on_failure: bool
    dispatch: Dispatch
    user_id: str
    created_at: str
    updated_at: str


@dataclass(frozen=True)
class StepView:
    """Read-only projection of a step."""

    step_id: str
    dispatch_id: str
    step_type: StepType
    step_sequence: int
    lane: Lane | None
    status: StepStatus
    worker_id: str | None
    result_json: str | None
    error: str | None
    retry_count: int
    created_at: str
    updated_at: str


@dataclass(frozen=True)
class QueuedStep:
    """A step claimed from the job queue."""

    step_id: str
    dispatch_id: str
    step_type: StepType
    step_sequence: int
    lane: Lane | None
    payload_json: str


@dataclass(frozen=True)
class DispatchListFilter:
    """Filter criteria for dispatch queries."""

    status: DispatchStatus | None = None
    lane: Lane | None = None
    project: str | None = None
    user_id: str | None = None
    since: str | None = None
    until: str | None = None
    limit: int = field(default=50)
    offset: int = field(default=0)


@dataclass(frozen=True)
class EventRow:
    """Single event row from the database."""

    id: int
    timestamp: str
    entity_id: str
    entity_type: str
    event_type: str
    payload: dict[str, JsonValue]


@dataclass(frozen=True)
class EventQueryResult:
    """Result of an event query with pagination metadata."""

    events: list[EventRow] = field(default_factory=list)
    total: int = 0
    skipped: int = 0
