"""Enums for the event-sourced dispatch lifecycle."""

from __future__ import annotations

from enum import StrEnum
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from tanren_core.schemas import Cli


class EntityType(StrEnum):
    """Entity types for the polymorphic events table."""

    DISPATCH = "dispatch"
    USER = "user"
    API_KEY = "api_key"


class DispatchMode(StrEnum):
    """How a dispatch's steps are managed."""

    AUTO = "auto"
    MANUAL = "manual"


class StepType(StrEnum):
    """Step types within a dispatch lifecycle."""

    PROVISION = "provision"
    EXECUTE = "execute"
    TEARDOWN = "teardown"
    DRY_RUN = "dry_run"


class Lane(StrEnum):
    """Concurrency lanes for execute steps."""

    IMPL = "impl"
    AUDIT = "audit"
    GATE = "gate"


class DispatchStatus(StrEnum):
    """Overall status of a dispatch."""

    PENDING = "pending"
    RUNNING = "running"
    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"


class StepStatus(StrEnum):
    """Status of a single step in the queue."""

    PENDING = "pending"
    RUNNING = "running"
    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"


CLI_LANE_MAP: dict[str, Lane] = {
    "opencode": Lane.IMPL,
    "claude": Lane.IMPL,
    "codex": Lane.AUDIT,
    "bash": Lane.GATE,
}


def cli_to_lane(cli: Cli) -> Lane:
    """Map a CLI tool to its concurrency lane.

    Returns:
        The lane corresponding to the given CLI tool.
    """
    return CLI_LANE_MAP[str(cli)]
