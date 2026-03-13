"""API-specific request and response models.

Core domain models (Dispatch, Result, VMAssignment, etc.) are reused directly
from tanren_core. Only models that are genuinely API-specific live here.
"""

from pydantic import BaseModel, ConfigDict, Field

from tanren_core.schemas import Cli, Phase


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


class DispatchRequest(BaseModel):
    """Submit a dispatch — omits workflow_id (auto-generated server-side)."""

    model_config = ConfigDict(extra="forbid")

    project: str = Field(..., description="Project name matching repo name")
    phase: Phase = Field(..., description="Dispatch phase type")
    branch: str = Field(..., description="Git branch name")
    spec_folder: str = Field(..., description="Relative path to spec folder")
    cli: Cli = Field(..., description="CLI tool to use")
    model: str | None = Field(default=None, description="Model identifier")
    timeout: int = Field(default=1800, ge=1, description="Max execution time in seconds")
    environment_profile: str = Field(default="default", description="Environment profile name")
    context: str | None = Field(default=None, description="Extra context for the agent")
    gate_cmd: str | None = Field(default=None, description="Shell command for gate phases")


class ProvisionRequest(BaseModel):
    """Request to provision a VM — subset of Dispatch fields."""

    model_config = ConfigDict(extra="forbid")

    project: str = Field(..., description="Project name")
    branch: str = Field(..., description="Git branch")
    environment_profile: str = Field(default="default", description="Environment profile")


class RunFullRequest(BaseModel):
    """Full lifecycle request — combines provision + execute fields."""

    model_config = ConfigDict(extra="forbid")

    project: str = Field(..., description="Project name")
    branch: str = Field(..., description="Git branch")
    spec_path: str = Field(..., description="Spec folder path")
    phase: Phase = Field(..., description="Phase to execute")
    environment_profile: str = Field(default="default", description="Environment profile")
    timeout: int = Field(default=1800, ge=1, description="Max execution seconds")
    context: str | None = Field(default=None, description="Extra context")
    gate_cmd: str | None = Field(default=None, description="Gate command")


class DispatchAccepted(BaseModel):
    """Thin response confirming dispatch acceptance."""

    model_config = ConfigDict(extra="forbid")

    dispatch_id: str = Field(..., description="Auto-generated workflow identifier")
    status: str = Field(default="accepted", description="Acceptance status")


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


class PaginatedEvents(BaseModel):
    """Pagination wrapper for event queries."""

    model_config = ConfigDict(extra="forbid")

    events: list[dict] = Field(default_factory=list, description="Event records (raw payloads)")
    total: int = Field(..., description="Total matching events")
    limit: int = Field(..., description="Page size")
    offset: int = Field(..., description="Current offset")
