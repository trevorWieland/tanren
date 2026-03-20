"""Data types for remote execution adapters."""

from __future__ import annotations

from enum import StrEnum

from pydantic import BaseModel, ConfigDict, Field


class VMProvider(StrEnum):
    """Supported VM providers."""

    MANUAL = "manual"
    HETZNER = "hetzner"
    GCP = "gcp"


class VMRequirements(BaseModel):
    """Resource requirements for VM provisioning."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    profile: str = Field(..., description="Environment profile name for this VM request")
    cpu: int = Field(default=2, ge=1, description="Minimum number of CPU cores")
    memory_gb: int = Field(default=4, ge=1, description="Minimum memory in gigabytes")
    gpu: bool = Field(default=False, description="Whether a GPU is required")
    server_type: str | None = Field(
        default=None, description="Provider-specific server type override"
    )
    labels: dict[str, str] = Field(
        default_factory=dict, description="Labels to apply to the provisioned VM"
    )


class VMHandle(BaseModel):
    """Handle to a provisioned VM."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    vm_id: str = Field(..., description="Unique VM identifier")
    host: str = Field(..., description="VM host address (IP or hostname)")
    provider: VMProvider = Field(
        default=VMProvider.MANUAL, description="Cloud provider that manages this VM"
    )
    created_at: str = Field(..., description="ISO 8601 timestamp when the VM was created")
    labels: dict[str, str] = Field(
        default_factory=dict, description="Key-value labels attached to the VM"
    )
    hourly_cost: float | None = Field(
        default=None, ge=0.0, description="Estimated hourly cost in USD"
    )


class VMAssignment(BaseModel):
    """Record of a VM assigned to a workflow."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    vm_id: str = Field(..., description="Unique VM identifier")
    workflow_id: str = Field(..., description="Workflow that this VM is assigned to")
    project: str = Field(..., description="Target project name")
    spec: str = Field(..., description="Environment spec or profile name used")
    host: str = Field(..., description="VM host address (IP or hostname)")
    assigned_at: str = Field(..., description="ISO 8601 timestamp when the VM was assigned")


class WorkspaceSpec(BaseModel):
    """Specification for setting up a remote workspace."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    project: str = Field(..., description="Target project name")
    repo_url: str = Field(..., description="Git repository URL to clone")
    branch: str = Field(..., description="Git branch to check out")
    setup_commands: tuple[str, ...] = Field(
        default_factory=tuple, description="Shell commands to run after cloning"
    )
    teardown_commands: tuple[str, ...] = Field(
        default_factory=tuple, description="Shell commands to run before cleanup"
    )


class WorkspacePath(BaseModel):
    """Path to a remote workspace."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    path: str = Field(..., description="Absolute path to the workspace on the remote VM")
    project: str = Field(..., description="Target project name")
    branch: str = Field(..., description="Git branch checked out in the workspace")


class SecretBundle(BaseModel):
    """Grouped secrets for remote injection."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    developer: dict[str, str] = Field(
        default_factory=dict, description="Developer-scoped secrets (key-value pairs)"
    )
    project: dict[str, str] = Field(
        default_factory=dict, description="Project-scoped secrets written to .env"
    )
    infrastructure: dict[str, str] = Field(
        default_factory=dict, description="Infrastructure-scoped secrets (e.g. cloud credentials)"
    )


class BootstrapResult(BaseModel):
    """Result of VM bootstrapping."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    installed: tuple[str, ...] = Field(
        default_factory=tuple, description="Tools that were installed"
    )
    skipped: tuple[str, ...] = Field(
        default_factory=tuple, description="Tools that were already present and skipped"
    )
    duration_secs: int = Field(default=0, ge=0, description="Bootstrap duration in seconds")


class RemoteResult(BaseModel):
    """Result of a remote command execution."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    exit_code: int = Field(..., description="Process exit code")
    stdout: str = Field(default="", description="Standard output content")
    stderr: str = Field(default="", description="Standard error content")
    timed_out: bool = Field(default=False, description="Whether the command exceeded its timeout")


class RemoteAgentResult(BaseModel):
    """Result of a remote agent execution."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    exit_code: int = Field(..., description="Agent process exit code")
    stdout: str = Field(default="", description="Standard output content")
    timed_out: bool = Field(..., description="Whether the agent exceeded its timeout")
    duration_secs: int = Field(..., ge=0, description="Agent execution duration in seconds")
    stderr: str = Field(default="", description="Standard error content")
    signal_content: str = Field(default="", description="Content of the signal file if present")
