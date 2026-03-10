"""Data types for remote execution adapters."""

from __future__ import annotations

from dataclasses import dataclass, field


@dataclass(frozen=True)
class VMRequirements:
    """Resource requirements for VM provisioning."""

    profile: str
    cpu: int = 2
    memory_gb: int = 4
    gpu: bool = False
    labels: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class VMHandle:
    """Handle to a provisioned VM."""

    vm_id: str
    host: str
    provider: str
    created_at: str
    labels: dict[str, str] = field(default_factory=dict)
    hourly_cost: float | None = None


@dataclass(frozen=True)
class VMAssignment:
    """Record of a VM assigned to a workflow."""

    vm_id: str
    workflow_id: str
    project: str
    spec: str
    host: str
    assigned_at: str


@dataclass(frozen=True)
class WorkspaceSpec:
    """Specification for setting up a remote workspace."""

    project: str
    repo_url: str
    branch: str
    setup_commands: tuple[str, ...] = ()
    teardown_commands: tuple[str, ...] = ()


@dataclass(frozen=True)
class WorkspacePath:
    """Path to a remote workspace."""

    path: str
    project: str
    branch: str


@dataclass(frozen=True)
class SecretBundle:
    """Grouped secrets for remote injection."""

    developer: dict[str, str] = field(default_factory=dict)
    project: dict[str, str] = field(default_factory=dict)
    infrastructure: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class BootstrapResult:
    """Result of VM bootstrapping."""

    installed: tuple[str, ...] = ()
    skipped: tuple[str, ...] = ()
    duration_secs: int = 0


@dataclass(frozen=True)
class RemoteResult:
    """Result of a remote command execution."""

    exit_code: int
    stdout: str
    stderr: str
    timed_out: bool = False


@dataclass(frozen=True)
class RemoteAgentResult:
    """Result of a remote agent execution."""

    exit_code: int
    stdout: str
    timed_out: bool
    duration_secs: int
    stderr: str = ""
    signal_content: str = ""
