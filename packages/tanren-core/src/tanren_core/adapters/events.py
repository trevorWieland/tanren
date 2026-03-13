"""Structured event types emitted during dispatch handling."""

from __future__ import annotations

from typing import Literal

from pydantic import BaseModel, ConfigDict, Field

from tanren_core.adapters.remote_types import VMProvider
from tanren_core.postflight import IntegrityRepairs


class Event(BaseModel):
    """Base event with common fields."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    timestamp: str = Field(..., description="ISO 8601 timestamp")
    workflow_id: str = Field(...)


class DispatchReceived(Event):
    """A dispatch was received and processing begins."""

    type: Literal["dispatch_received"] = Field(
        default="dispatch_received", description="Event type discriminator"
    )
    phase: str = Field(...)
    project: str = Field(...)
    cli: str = Field(...)


class PhaseStarted(Event):
    """An agent/gate process is about to be spawned."""

    type: Literal["phase_started"] = Field(
        default="phase_started", description="Event type discriminator"
    )
    phase: str = Field(...)
    worktree_path: str = Field(...)


class PhaseCompleted(Event):
    """A phase finished (successfully or not)."""

    type: Literal["phase_completed"] = Field(
        default="phase_completed", description="Event type discriminator"
    )
    phase: str = Field(...)
    outcome: str = Field(...)
    signal: str | None = Field(default=None)
    duration_secs: int = Field(..., ge=0)
    exit_code: int = Field(...)


class PreflightCompleted(Event):
    """Pre-flight checks finished."""

    type: Literal["preflight_completed"] = Field(
        default="preflight_completed", description="Event type discriminator"
    )
    passed: bool = Field(...)
    repairs: list[str] = Field(default_factory=list)


class PostflightCompleted(Event):
    """Post-flight integrity checks finished."""

    type: Literal["postflight_completed"] = Field(
        default="postflight_completed", description="Event type discriminator"
    )
    phase: str = Field(...)
    pushed: bool | None = Field(default=None)
    integrity_repairs: IntegrityRepairs = Field(default_factory=IntegrityRepairs)


class ErrorOccurred(Event):
    """An unhandled error occurred during dispatch handling."""

    type: Literal["error_occurred"] = Field(
        default="error_occurred", description="Event type discriminator"
    )
    phase: str = Field(...)
    error: str = Field(...)
    error_class: str | None = Field(default=None)


class RetryScheduled(Event):
    """A transient error triggered a retry."""

    type: Literal["retry_scheduled"] = Field(
        default="retry_scheduled", description="Event type discriminator"
    )
    phase: str = Field(...)
    attempt: int = Field(..., ge=1)
    max_attempts: int = Field(..., ge=1)
    backoff_secs: int = Field(..., ge=0)


class VMProvisioned(Event):
    """A VM was provisioned for a workflow."""

    type: Literal["vm_provisioned"] = Field(
        default="vm_provisioned", description="Event type discriminator"
    )
    vm_id: str = Field(...)
    host: str = Field(...)
    provider: VMProvider = Field(...)
    project: str = Field(...)
    profile: str = Field(...)
    hourly_cost: float | None = Field(default=None, ge=0.0)


class VMReleased(Event):
    """A VM was released after workflow completion."""

    type: Literal["vm_released"] = Field(
        default="vm_released", description="Event type discriminator"
    )
    vm_id: str = Field(...)
    duration_secs: int = Field(..., ge=0)
    estimated_cost: float | None = Field(default=None, ge=0.0)


class BootstrapCompleted(Event):
    """VM bootstrap finished."""

    type: Literal["bootstrap_completed"] = Field(
        default="bootstrap_completed", description="Event type discriminator"
    )
    vm_id: str = Field(...)
    installed: list[str] = Field(default_factory=list)
    skipped: list[str] = Field(default_factory=list)
    duration_secs: int = Field(default=0, ge=0)
