"""API-specific request and response models.

Core domain models (Dispatch, Result, VMAssignment, etc.) are reused directly
from tanren_core. Only models that are genuinely API-specific live here.
"""

from enum import StrEnum
from typing import Annotated

from pydantic import BaseModel, ConfigDict, Field

from tanren_core.adapters.events import (
    BootstrapCompleted,
    DispatchReceived,
    ErrorOccurred,
    PhaseCompleted,
    PhaseStarted,
    PostflightCompleted,
    PreflightCompleted,
    RetryScheduled,
    TokenUsageRecorded,
    VMProvisioned,
    VMReleased,
)
from tanren_core.adapters.remote_types import VMProvider, VMRequirements
from tanren_core.roles import AuthMode
from tanren_core.schemas import Cli, Outcome, Phase

# ---------------------------------------------------------------------------
# Enums
# ---------------------------------------------------------------------------


class DispatchRunStatus(StrEnum):
    """Runtime status of a dispatch."""

    PENDING = "pending"
    RUNNING = "running"
    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"


class VMStatus(StrEnum):
    """Lifecycle status of a VM."""

    ACTIVE = "active"
    PROVISIONING = "provisioning"
    FAILED = "failed"
    RELEASING = "releasing"
    RELEASED = "released"


class RunEnvironmentStatus(StrEnum):
    """Lifecycle status of a run environment."""

    PROVISIONING = "provisioning"
    PROVISIONED = "provisioned"
    EXECUTING = "executing"
    TEARING_DOWN = "tearing_down"
    COMPLETED = "completed"
    FAILED = "failed"


# ---------------------------------------------------------------------------
# Request models
# ---------------------------------------------------------------------------


class DispatchRequest(BaseModel):
    """Submit a dispatch — omits workflow_id (auto-generated server-side)."""

    model_config = ConfigDict(extra="forbid")

    project: str = Field(..., description="Project name matching repo name")
    phase: Phase = Field(..., description="Dispatch phase type")
    branch: str = Field(..., description="Git branch name")
    spec_folder: str = Field(..., description="Relative path to spec folder")
    cli: Cli = Field(..., description="CLI tool to use")
    auth: AuthMode = Field(default=AuthMode.API_KEY, description="Authentication mode")
    model: str | None = Field(default=None, description="Model identifier")
    timeout: int = Field(default=1800, ge=1, description="Max execution time in seconds")
    environment_profile: str = Field(default="default", description="Environment profile name")
    context: str | None = Field(default=None, description="Extra context for the agent")
    gate_cmd: str | None = Field(default=None, description="Shell command for gate phases")
    issue: str = Field(
        default="0",
        min_length=1,
        pattern=r"^[A-Za-z0-9][A-Za-z0-9_-]*$",
        description="Issue identifier ('0' = API-originated)",
    )


class ProvisionRequest(BaseModel):
    """Request to provision a VM — subset of Dispatch fields."""

    model_config = ConfigDict(extra="forbid")

    project: str = Field(..., description="Project name")
    branch: str = Field(..., description="Git branch")
    environment_profile: str = Field(default="default", description="Environment profile")


class ExecuteRequest(BaseModel):
    """Request body for POST /run/{env_id}/execute."""

    model_config = ConfigDict(extra="forbid")

    project: str = Field(..., description="Project name")
    spec_path: str = Field(..., description="Spec folder path")
    phase: Phase = Field(..., description="Phase to execute")
    cli: Cli = Field(..., description="CLI tool")
    auth: AuthMode = Field(..., description="Authentication mode")
    model: str | None = Field(default=None, description="Model identifier")
    timeout: int = Field(default=1800, ge=1, description="Max execution seconds")
    context: str | None = Field(default=None, description="Extra context")
    gate_cmd: str | None = Field(default=None, description="Gate command")


class RunFullRequest(BaseModel):
    """Full lifecycle request — combines provision + execute fields."""

    model_config = ConfigDict(extra="forbid")

    project: str = Field(..., description="Project name")
    branch: str = Field(..., description="Git branch")
    spec_path: str = Field(..., description="Spec folder path")
    phase: Phase = Field(..., description="Phase to execute")
    cli: Cli = Field(..., description="CLI tool")
    auth: AuthMode = Field(..., description="Authentication mode")
    environment_profile: str = Field(default="default", description="Environment profile")
    timeout: int = Field(default=1800, ge=1, description="Max execution seconds")
    context: str | None = Field(default=None, description="Extra context")
    gate_cmd: str | None = Field(default=None, description="Gate command")


# ---------------------------------------------------------------------------
# Response models — general
# ---------------------------------------------------------------------------


class ErrorResponse(BaseModel):
    """Consistent error envelope for API error responses."""

    model_config = ConfigDict(extra="forbid")

    detail: str = Field(..., description="Human-readable error message")
    error_code: str = Field(..., description="Machine-readable error code")
    timestamp: str = Field(..., description="ISO 8601 timestamp")
    request_id: str | None = Field(default=None, description="Request correlation ID")


class HealthResponse(BaseModel):
    """Service health and version info."""

    model_config = ConfigDict(extra="forbid")

    status: str = Field(..., description="Service status")
    version: str = Field(..., description="Service version")
    uptime_seconds: float = Field(..., description="Seconds since service start")


class ReadinessResponse(BaseModel):
    """Readiness probe response."""

    model_config = ConfigDict(extra="forbid")

    status: str = Field(..., description="Readiness indicator")


class ConfigResponse(BaseModel):
    """Non-secret config projection."""

    model_config = ConfigDict(extra="forbid")

    ipc_dir: str = Field(..., description="IPC directory path")
    github_dir: str = Field(..., description="Root directory for git repos")
    poll_interval: float = Field(..., description="Seconds between polls")
    heartbeat_interval: float = Field(..., description="Seconds between heartbeats")
    max_opencode: int = Field(..., description="Max concurrent opencode processes")
    max_codex: int = Field(..., description="Max concurrent codex processes")
    max_gate: int = Field(..., description="Max concurrent gate processes")
    events_enabled: bool = Field(..., description="Whether event emission is active")
    remote_enabled: bool = Field(..., description="Whether remote execution is configured")


class DispatchAccepted(BaseModel):
    """Thin response confirming dispatch acceptance."""

    model_config = ConfigDict(extra="forbid")

    dispatch_id: str = Field(..., description="Auto-generated workflow identifier")
    status: str = Field(default="accepted", description="Acceptance status")


# ---------------------------------------------------------------------------
# Response models — dispatch
# ---------------------------------------------------------------------------


class DispatchDetail(BaseModel):
    """Full dispatch detail including runtime tracking."""

    model_config = ConfigDict(extra="forbid")

    workflow_id: str = Field(..., description="Unique workflow identifier")
    phase: Phase = Field(..., description="Dispatch phase type")
    project: str = Field(..., description="Project name")
    spec_folder: str = Field(..., description="Relative path to spec folder")
    branch: str = Field(..., description="Git branch name")
    cli: Cli = Field(..., description="CLI tool used")
    auth: AuthMode = Field(default=AuthMode.API_KEY, description="Authentication mode")
    model: str | None = Field(default=None, description="Model identifier")
    timeout: int = Field(..., ge=1, description="Max execution time in seconds")
    environment_profile: str = Field(..., description="Environment profile name")
    context: str | None = Field(default=None, description="Extra context for the agent")
    gate_cmd: str | None = Field(default=None, description="Shell command for gate phases")
    status: DispatchRunStatus = Field(..., description="Current dispatch status")
    outcome: Outcome | None = Field(default=None, description="Final outcome if completed")
    created_at: str = Field(..., description="ISO 8601 creation timestamp")
    started_at: str | None = Field(default=None, description="ISO 8601 start timestamp")
    completed_at: str | None = Field(default=None, description="ISO 8601 completion timestamp")


class DispatchCancelled(BaseModel):
    """Confirmation that a dispatch was cancelled."""

    model_config = ConfigDict(extra="forbid")

    dispatch_id: str = Field(..., description="Cancelled workflow identifier")
    status: DispatchRunStatus = Field(
        default=DispatchRunStatus.CANCELLED, description="Cancellation status"
    )


# ---------------------------------------------------------------------------
# Response models — VM
# ---------------------------------------------------------------------------


class VMSummary(BaseModel):
    """Summary of a VM assignment."""

    model_config = ConfigDict(extra="forbid")

    vm_id: str = Field(..., description="VM identifier")
    host: str = Field(..., description="VM hostname or IP")
    provider: VMProvider = Field(..., description="VM provider")
    workflow_id: str | None = Field(default=None, description="Associated workflow ID")
    project: str | None = Field(default=None, description="Associated project name")
    status: VMStatus = Field(..., description="Current VM status")
    created_at: str = Field(..., description="ISO 8601 creation timestamp")


class VMReleaseConfirmed(BaseModel):
    """Confirmation that a VM was released."""

    model_config = ConfigDict(extra="forbid")

    vm_id: str = Field(..., description="Released VM identifier")
    status: VMStatus = Field(default=VMStatus.RELEASED, description="Release status")


class VMProvisionAccepted(BaseModel):
    """Accepted response for async VM provisioning."""

    model_config = ConfigDict(extra="forbid")

    env_id: str = Field(..., description="Provisioning tracking identifier")
    status: VMStatus = Field(default=VMStatus.PROVISIONING, description="Provisioning status")


class VMProvisionStatus(BaseModel):
    """Status of an in-progress or completed VM provisioning."""

    model_config = ConfigDict(extra="forbid")

    env_id: str = Field(..., description="Provisioning tracking identifier")
    status: VMStatus = Field(..., description="Current provisioning status")
    vm_id: str | None = Field(default=None, description="VM identifier (set once provisioned)")
    host: str | None = Field(default=None, description="VM hostname or IP (set once provisioned)")
    provider: VMProvider | None = Field(default=None, description="VM provider")
    created_at: str | None = Field(default=None, description="ISO 8601 creation timestamp")


class VMDryRunResult(BaseModel):
    """Result of a VM dry-run provisioning check."""

    model_config = ConfigDict(extra="forbid")

    provider: VMProvider = Field(..., description="VM provider that would be used")
    server_type: str | None = Field(default=None, description="Server type that would be used")
    estimated_cost_hourly: float | None = Field(
        default=None, ge=0.0, description="Estimated hourly cost"
    )
    would_provision: bool = Field(..., description="Whether provisioning would proceed")
    requirements: VMRequirements = Field(..., description="Resolved VM requirements")


# ---------------------------------------------------------------------------
# Response models — run
# ---------------------------------------------------------------------------


class RunEnvironment(BaseModel):
    """Provisioned run environment handle."""

    model_config = ConfigDict(extra="forbid")

    env_id: str = Field(..., description="Environment identifier")
    vm_id: str = Field(..., description="Backing VM identifier")
    host: str = Field(..., description="VM hostname or IP")
    status: RunEnvironmentStatus = Field(
        default=RunEnvironmentStatus.PROVISIONED, description="Environment status"
    )


class RunExecuteAccepted(BaseModel):
    """Confirmation that execution was accepted."""

    model_config = ConfigDict(extra="forbid")

    env_id: str = Field(..., description="Environment identifier")
    dispatch_id: str = Field(..., description="Dispatch workflow identifier")
    status: RunEnvironmentStatus = Field(
        default=RunEnvironmentStatus.EXECUTING, description="Execution status"
    )


class RunTeardownAccepted(BaseModel):
    """Confirmation that teardown was accepted."""

    model_config = ConfigDict(extra="forbid")

    env_id: str = Field(..., description="Environment identifier")
    status: RunEnvironmentStatus = Field(
        default=RunEnvironmentStatus.TEARING_DOWN, description="Teardown status"
    )


class RunStatus(BaseModel):
    """Status of a running environment."""

    model_config = ConfigDict(extra="forbid")

    env_id: str = Field(..., description="Environment identifier")
    status: RunEnvironmentStatus = Field(..., description="Current environment status")
    phase: Phase | None = Field(default=None, description="Current phase if executing")
    outcome: Outcome | None = Field(default=None, description="Final outcome if completed")
    started_at: str | None = Field(default=None, description="ISO 8601 start timestamp")
    duration_secs: int | None = Field(default=None, ge=0, description="Elapsed seconds")
    vm_id: str | None = Field(default=None, description="Backing VM identifier")
    host: str | None = Field(default=None, description="VM hostname or IP")


# ---------------------------------------------------------------------------
# Response models — metrics
# ---------------------------------------------------------------------------


class CostGroupBy(StrEnum):
    """Grouping options for cost metrics."""

    MODEL = "model"
    DAY = "day"
    WORKFLOW = "workflow"


class MetricsSummaryResponse(BaseModel):
    """Workflow execution summary metrics."""

    model_config = ConfigDict(extra="forbid")

    total_phases: int = Field(..., description="Total phase completions")
    succeeded: int = Field(..., description="Phases with success outcome")
    failed: int = Field(..., description="Phases with fail outcome")
    errored: int = Field(..., description="Phases with error outcome")
    timed_out: int = Field(..., description="Phases with timeout outcome")
    blocked: int = Field(..., description="Phases with blocked outcome")
    success_rate: float = Field(..., ge=0.0, le=1.0, description="Fraction succeeded / total")
    avg_duration_secs: float = Field(..., description="Mean phase duration in seconds")
    p50_duration_secs: float = Field(..., description="Median phase duration in seconds")
    p95_duration_secs: float = Field(..., description="95th percentile duration in seconds")


class CostBucketResponse(BaseModel):
    """Single aggregation bucket for cost metrics."""

    model_config = ConfigDict(extra="forbid")

    group_key: str = Field(..., description="Grouping key (model list, date, or workflow_id)")
    total_cost: float = Field(..., ge=0.0, description="Total spend in USD")
    total_tokens: int = Field(..., ge=0, description="Total tokens consumed")
    input_tokens: int = Field(..., ge=0)
    output_tokens: int = Field(..., ge=0)
    cache_read_tokens: int = Field(default=0, ge=0)
    cache_creation_tokens: int = Field(default=0, ge=0)
    reasoning_tokens: int = Field(default=0, ge=0)
    event_count: int = Field(..., ge=0, description="Number of token usage events")


class MetricsCostsResponse(BaseModel):
    """Aggregated token cost metrics."""

    model_config = ConfigDict(extra="forbid")

    buckets: list[CostBucketResponse] = Field(default_factory=list)
    total_cost: float = Field(..., ge=0.0, description="Grand total cost across all buckets")
    total_tokens: int = Field(..., ge=0, description="Grand total tokens across all buckets")
    group_by: str = Field(..., description="Grouping used (model/day/workflow)")


class MetricsVMsResponse(BaseModel):
    """Aggregated VM utilization metrics."""

    model_config = ConfigDict(extra="forbid")

    total_provisioned: int = Field(..., ge=0)
    total_released: int = Field(..., ge=0)
    currently_active: int = Field(..., ge=0)
    total_vm_duration_secs: int = Field(..., ge=0)
    total_estimated_cost: float = Field(..., ge=0.0)
    avg_duration_secs: float = Field(..., ge=0.0)
    by_provider: dict[str, int] = Field(
        default_factory=dict, description="Provisioned count per provider"
    )


# ---------------------------------------------------------------------------
# Events — discriminated union
# ---------------------------------------------------------------------------

EventPayload = Annotated[
    DispatchReceived
    | PhaseStarted
    | PhaseCompleted
    | PreflightCompleted
    | PostflightCompleted
    | ErrorOccurred
    | RetryScheduled
    | VMProvisioned
    | VMReleased
    | BootstrapCompleted
    | TokenUsageRecorded,
    Field(discriminator="type"),
]


class PaginatedEvents(BaseModel):
    """Pagination wrapper for event queries."""

    model_config = ConfigDict(extra="forbid")

    events: list[EventPayload] = Field(default_factory=list, description="Typed event records")
    total: int = Field(..., description="Total matching events in database (includes unparseable)")
    limit: int = Field(..., description="Page size")
    offset: int = Field(..., description="Current offset")
    skipped: int = Field(
        default=0, ge=0, description="Events skipped due to parse errors in this page"
    )
