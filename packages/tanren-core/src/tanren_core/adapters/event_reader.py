"""Event data types used by store protocols and SQLite store."""

from __future__ import annotations

from dataclasses import dataclass, field


@dataclass(frozen=True)
class EventRow:
    """Single event row from the database."""

    id: int
    timestamp: str
    workflow_id: str
    event_type: str
    payload: dict[str, object]


@dataclass(frozen=True)
class EventQueryResult:
    """Result of an event query with pagination metadata."""

    events: list[EventRow] = field(default_factory=list)
    total: int = 0
    skipped: int = 0
