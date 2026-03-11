"""Structured event types emitted during dispatch handling."""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict, Field

from worker_manager.adapters.remote_types import VMProvider
from worker_manager.postflight import IntegrityRepairs


class Event(BaseModel):
    """Base event with common fields."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    timestamp: str = Field(..., description="ISO 8601 timestamp")
    workflow_id: str = Field(...)


class DispatchReceived(Event):
    """A dispatch was received and processing begins."""

    phase: str = Field(...)
    project: str = Field(...)
    cli: str = Field(...)


class PhaseStarted(Event):
    """An agent/gate process is about to be spawned."""

    phase: str = Field(...)
    worktree_path: str = Field(...)


class PhaseCompleted(Event):
    """A phase finished (successfully or not)."""

    phase: str = Field(...)
    outcome: str = Field(...)
    signal: str | None = Field(default=None)
    duration_secs: int = Field(..., ge=0)
    exit_code: int = Field(...)


class PreflightCompleted(Event):
    """Pre-flight checks finished."""

    passed: bool = Field(...)
    repairs: list[str] = Field(default_factory=list)


class PostflightCompleted(Event):
    """Post-flight integrity checks finished."""

    phase: str = Field(...)
    pushed: bool | None = Field(default=None)
    integrity_repairs: IntegrityRepairs = Field(default_factory=IntegrityRepairs)


class ErrorOccurred(Event):
    """An unhandled error occurred during dispatch handling."""

    phase: str = Field(...)
    error: str = Field(...)
    error_class: str | None = Field(default=None)


class RetryScheduled(Event):
    """A transient error triggered a retry."""

    phase: str = Field(...)
    attempt: int = Field(..., ge=1)
    max_attempts: int = Field(..., ge=1)
    backoff_secs: int = Field(..., ge=0)


class VMProvisioned(Event):
    """A VM was provisioned for a workflow."""

    vm_id: str = Field(...)
    host: str = Field(...)
    provider: VMProvider = Field(...)
    project: str = Field(...)
    profile: str = Field(...)
    hourly_cost: float | None = Field(default=None, ge=0.0)


class VMReleased(Event):
    """A VM was released after workflow completion."""

    vm_id: str = Field(...)
    duration_secs: int = Field(..., ge=0)
    estimated_cost: float | None = Field(default=None, ge=0.0)


class BootstrapCompleted(Event):
    """VM bootstrap finished."""

    vm_id: str = Field(...)
    installed: list[str] = Field(default_factory=list)
    skipped: list[str] = Field(default_factory=list)
    duration_secs: int = Field(default=0, ge=0)
