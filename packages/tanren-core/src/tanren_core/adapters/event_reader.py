"""Event data types used by store protocols and SQLite store."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from pydantic import JsonValue


@dataclass(frozen=True)
class EventRow:
    """Single event row from the database."""

    id: int
    timestamp: str
    workflow_id: str
    event_type: str
    payload: dict[str, JsonValue]


@dataclass(frozen=True)
class EventQueryResult:
    """Result of an event query with pagination metadata."""

    events: list[EventRow] = field(default_factory=list)
    total: int = 0
    skipped: int = 0
