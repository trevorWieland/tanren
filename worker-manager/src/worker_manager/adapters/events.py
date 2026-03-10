"""Structured event types emitted during dispatch handling."""

from __future__ import annotations

from dataclasses import dataclass, field


@dataclass
class Event:
    """Base event with common fields."""

    timestamp: str  # ISO 8601
    workflow_id: str


@dataclass
class DispatchReceived(Event):
    """A dispatch was received and processing begins."""

    phase: str
    project: str
    cli: str


@dataclass
class PhaseStarted(Event):
    """An agent/gate process is about to be spawned."""

    phase: str
    worktree_path: str


@dataclass
class PhaseCompleted(Event):
    """A phase finished (successfully or not)."""

    phase: str
    outcome: str
    signal: str | None
    duration_secs: int
    exit_code: int


@dataclass
class PreflightCompleted(Event):
    """Pre-flight checks finished."""

    passed: bool
    repairs: list[str] = field(default_factory=list)


@dataclass
class PostflightCompleted(Event):
    """Post-flight integrity checks finished."""

    phase: str
    pushed: bool | None
    integrity_repairs: dict = field(default_factory=dict)


@dataclass
class ErrorOccurred(Event):
    """An unhandled error occurred during dispatch handling."""

    phase: str
    error: str
    error_class: str | None = None


@dataclass
class RetryScheduled(Event):
    """A transient error triggered a retry."""

    phase: str
    attempt: int
    max_attempts: int
    backoff_secs: int
