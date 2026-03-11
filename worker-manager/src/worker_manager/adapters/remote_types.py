"""Data types for remote execution adapters."""

from __future__ import annotations

from enum import StrEnum

from pydantic import BaseModel, ConfigDict, Field


class VMProvider(StrEnum):
    """Supported VM providers."""

    MANUAL = "manual"
    HETZNER = "hetzner"


class VMRequirements(BaseModel):
    """Resource requirements for VM provisioning."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    profile: str = Field(...)
    cpu: int = Field(default=2, ge=1)
    memory_gb: int = Field(default=4, ge=1)
    gpu: bool = Field(default=False)
    server_type: str | None = Field(default=None)
    labels: dict[str, str] = Field(default_factory=dict)


class VMHandle(BaseModel):
    """Handle to a provisioned VM."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    vm_id: str = Field(...)
    host: str = Field(...)
    provider: VMProvider = Field(default=VMProvider.MANUAL)
    created_at: str = Field(...)
    labels: dict[str, str] = Field(default_factory=dict)
    hourly_cost: float | None = Field(default=None, ge=0.0)


class VMAssignment(BaseModel):
    """Record of a VM assigned to a workflow."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    vm_id: str = Field(...)
    workflow_id: str = Field(...)
    project: str = Field(...)
    spec: str = Field(...)
    host: str = Field(...)
    assigned_at: str = Field(...)


class WorkspaceSpec(BaseModel):
    """Specification for setting up a remote workspace."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    project: str = Field(...)
    repo_url: str = Field(...)
    branch: str = Field(...)
    setup_commands: tuple[str, ...] = Field(default_factory=tuple)
    teardown_commands: tuple[str, ...] = Field(default_factory=tuple)


class WorkspacePath(BaseModel):
    """Path to a remote workspace."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    path: str = Field(...)
    project: str = Field(...)
    branch: str = Field(...)


class SecretBundle(BaseModel):
    """Grouped secrets for remote injection."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    developer: dict[str, str] = Field(default_factory=dict)
    project: dict[str, str] = Field(default_factory=dict)
    infrastructure: dict[str, str] = Field(default_factory=dict)


class BootstrapResult(BaseModel):
    """Result of VM bootstrapping."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    installed: tuple[str, ...] = Field(default_factory=tuple)
    skipped: tuple[str, ...] = Field(default_factory=tuple)
    duration_secs: int = Field(default=0, ge=0)


class RemoteResult(BaseModel):
    """Result of a remote command execution."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    exit_code: int = Field(...)
    stdout: str = Field(default="")
    stderr: str = Field(default="")
    timed_out: bool = Field(default=False)


class RemoteAgentResult(BaseModel):
    """Result of a remote agent execution."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    exit_code: int = Field(...)
    stdout: str = Field(default="")
    timed_out: bool = Field(...)
    duration_secs: int = Field(..., ge=0)
    stderr: str = Field(default="")
    signal_content: str = Field(default="")
