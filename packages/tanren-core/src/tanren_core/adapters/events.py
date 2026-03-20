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
    workflow_id: str = Field(..., description="Unique identifier for the workflow run")


class DispatchReceived(Event):
    """A dispatch was received and processing begins."""

    type: Literal["dispatch_received"] = Field(
        default="dispatch_received", description="Event type discriminator"
    )
    phase: str = Field(..., description="Dispatch phase name (e.g. agent, gate)")
    project: str = Field(..., description="Target project name")
    cli: str = Field(..., description="CLI tool used for this dispatch")


class PhaseStarted(Event):
    """An agent/gate process is about to be spawned."""

    type: Literal["phase_started"] = Field(
        default="phase_started", description="Event type discriminator"
    )
    phase: str = Field(..., description="Phase being started")
    worktree_path: str = Field(..., description="Filesystem path to the git worktree")


class PhaseCompleted(Event):
    """A phase finished (successfully or not)."""

    type: Literal["phase_completed"] = Field(
        default="phase_completed", description="Event type discriminator"
    )
    phase: str = Field(..., description="Phase that completed")
    project: str = Field(..., description="Target project name")
    outcome: str = Field(..., description="Result outcome (e.g. success, failure, error)")
    signal: str | None = Field(default=None, description="Signal file content if present")
    duration_secs: int = Field(..., ge=0, description="Wall-clock duration in seconds")
    exit_code: int = Field(..., description="Process exit code")


class PreflightCompleted(Event):
    """Pre-flight checks finished."""

    type: Literal["preflight_completed"] = Field(
        default="preflight_completed", description="Event type discriminator"
    )
    passed: bool = Field(..., description="Whether all preflight checks passed")
    repairs: list[str] = Field(
        default_factory=list, description="List of repairs applied during preflight"
    )


class PostflightCompleted(Event):
    """Post-flight integrity checks finished."""

    type: Literal["postflight_completed"] = Field(
        default="postflight_completed", description="Event type discriminator"
    )
    phase: str = Field(..., description="Phase that was post-flighted")
    pushed: bool | None = Field(default=None, description="Whether changes were pushed to remote")
    integrity_repairs: IntegrityRepairs = Field(
        default_factory=IntegrityRepairs,
        description="Integrity repairs performed during postflight",
    )


class ErrorOccurred(Event):
    """An unhandled error occurred during dispatch handling."""

    type: Literal["error_occurred"] = Field(
        default="error_occurred", description="Event type discriminator"
    )
    phase: str = Field(..., description="Phase during which the error occurred")
    error: str = Field(..., description="Error message text")
    error_class: str | None = Field(
        default=None, description="Fully qualified exception class name"
    )


class RetryScheduled(Event):
    """A transient error triggered a retry."""

    type: Literal["retry_scheduled"] = Field(
        default="retry_scheduled", description="Event type discriminator"
    )
    phase: str = Field(..., description="Phase being retried")
    attempt: int = Field(..., ge=1, description="Current attempt number (1-based)")
    max_attempts: int = Field(..., ge=1, description="Maximum number of retry attempts")
    backoff_secs: int = Field(..., ge=0, description="Backoff delay before next attempt in seconds")


class VMProvisioned(Event):
    """A VM was provisioned for a workflow."""

    type: Literal["vm_provisioned"] = Field(
        default="vm_provisioned", description="Event type discriminator"
    )
    vm_id: str = Field(..., description="Unique VM identifier")
    host: str = Field(..., description="VM host address (IP or hostname)")
    provider: VMProvider = Field(..., description="Cloud provider that provisioned the VM")
    project: str = Field(..., description="Target project name")
    profile: str = Field(..., description="Environment profile used for provisioning")
    hourly_cost: float | None = Field(
        default=None, ge=0.0, description="Estimated hourly cost in USD"
    )


class VMReleased(Event):
    """A VM was released after workflow completion."""

    type: Literal["vm_released"] = Field(
        default="vm_released", description="Event type discriminator"
    )
    vm_id: str = Field(..., description="Unique VM identifier")
    project: str = Field(..., description="Target project name")
    duration_secs: int = Field(..., ge=0, description="Total VM usage duration in seconds")
    estimated_cost: float | None = Field(
        default=None, ge=0.0, description="Estimated total cost in USD"
    )


class BootstrapCompleted(Event):
    """VM bootstrap finished."""

    type: Literal["bootstrap_completed"] = Field(
        default="bootstrap_completed", description="Event type discriminator"
    )
    vm_id: str = Field(..., description="Unique VM identifier")
    installed: list[str] = Field(
        default_factory=list, description="Tools that were installed during bootstrap"
    )
    skipped: list[str] = Field(
        default_factory=list, description="Tools that were already installed and skipped"
    )
    duration_secs: int = Field(default=0, ge=0, description="Bootstrap duration in seconds")


class TokenUsageRecorded(Event):
    """Token usage data collected after a dispatch."""

    type: Literal["token_usage_recorded"] = Field(
        default="token_usage_recorded", description="Event type discriminator"
    )
    phase: str = Field(..., description="Phase that generated the token usage")
    project: str = Field(..., description="Target project name")
    cli: str = Field(..., description="CLI tool that consumed the tokens")
    input_tokens: int = Field(..., ge=0, description="Number of input tokens consumed")
    output_tokens: int = Field(..., ge=0, description="Number of output tokens generated")
    cache_creation_tokens: int = Field(
        default=0, ge=0, description="Tokens used to create prompt cache entries"
    )
    cache_read_tokens: int = Field(default=0, ge=0, description="Tokens read from prompt cache")
    cached_input_tokens: int = Field(default=0, ge=0, description="Input tokens served from cache")
    reasoning_tokens: int = Field(
        default=0, ge=0, description="Tokens used for chain-of-thought reasoning"
    )
    total_tokens: int = Field(..., ge=0, description="Total token count across all categories")
    total_cost: float = Field(..., ge=0.0, description="Estimated total cost in USD")
    models_used: list[str] = Field(
        default_factory=list, description="Model identifiers used during the session"
    )
    session_id: str | None = Field(default=None, description="CLI session identifier if available")
