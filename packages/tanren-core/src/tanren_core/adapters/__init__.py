"""Adapter interfaces and default implementations.

Protocols define the contract; concrete classes wrap existing module functions.
"""

from tanren_core.adapters.dotenv_provisioner import DotenvEnvProvisioner
from tanren_core.adapters.dotenv_validator import DotenvEnvValidator
from tanren_core.adapters.events import (
    BootstrapCompleted,
    DispatchReceived,
    ErrorOccurred,
    Event,
    PhaseCompleted,
    PhaseStarted,
    PostflightCompleted,
    PreflightCompleted,
    RetryScheduled,
    VMProvisioned,
    VMReleased,
)
from tanren_core.adapters.git_postflight import GitPostflightRunner
from tanren_core.adapters.git_preflight import GitPreflightRunner
from tanren_core.adapters.git_workspace import GitAuthConfig, GitWorkspaceManager
from tanren_core.adapters.git_worktree import GitWorktreeManager
from tanren_core.adapters.hetzner_vm import HetznerProvisionerSettings, HetznerVMProvisioner
from tanren_core.adapters.local_environment import LocalExecutionEnvironment
from tanren_core.adapters.manual_vm import (
    ManualProvisionerSettings,
    ManualVMConfig,
    ManualVMProvisioner,
    NoVMAvailableError,
)
from tanren_core.adapters.null_emitter import NullEventEmitter
from tanren_core.adapters.protocols import (
    EnvironmentBootstrapper,
    EnvProvisioner,
    EnvValidator,
    EventEmitter,
    ExecutionEnvironment,
    PostflightRunner,
    PreflightRunner,
    ProcessSpawner,
    RemoteConnection,
    VMStateStore,
    WorktreeManager,
)
from tanren_core.adapters.protocols import (
    VMProvisioner as VMProvisionerProtocol,
)
from tanren_core.adapters.protocols import (
    WorkspaceManager as WorkspaceManagerProtocol,
)
from tanren_core.adapters.remote_runner import RemoteAgentRunner
from tanren_core.adapters.remote_types import (
    BootstrapResult,
    RemoteAgentResult,
    RemoteResult,
    SecretBundle,
    VMAssignment,
    VMHandle,
    VMProvider,
    VMRequirements,
    WorkspacePath,
    WorkspaceSpec,
)
from tanren_core.adapters.sqlite_emitter import SqliteEventEmitter
from tanren_core.adapters.sqlite_vm_state import SqliteVMStateStore
from tanren_core.adapters.ssh import SSHConfig, SSHConnection
from tanren_core.adapters.ssh_environment import SSHExecutionEnvironment
from tanren_core.adapters.subprocess_spawner import SubprocessSpawner
from tanren_core.adapters.types import (
    AccessInfo,
    EnvironmentHandle,
    PhaseResult,
    ProvisionError,
)
from tanren_core.adapters.ubuntu_bootstrap import UbuntuBootstrapper

__all__ = [
    "AccessInfo",
    "BootstrapCompleted",
    "BootstrapResult",
    "DispatchReceived",
    "DotenvEnvProvisioner",
    "DotenvEnvValidator",
    "EnvProvisioner",
    "EnvValidator",
    "EnvironmentBootstrapper",
    "EnvironmentHandle",
    "ErrorOccurred",
    "Event",
    "EventEmitter",
    "ExecutionEnvironment",
    "GitAuthConfig",
    "GitPostflightRunner",
    "GitPreflightRunner",
    "GitWorkspaceManager",
    "GitWorktreeManager",
    "HetznerProvisionerSettings",
    "HetznerVMProvisioner",
    "LocalExecutionEnvironment",
    "ManualProvisionerSettings",
    "ManualVMConfig",
    "ManualVMProvisioner",
    "NoVMAvailableError",
    "NullEventEmitter",
    "PhaseCompleted",
    "PhaseResult",
    "PhaseStarted",
    "PostflightCompleted",
    "PostflightRunner",
    "PreflightCompleted",
    "PreflightRunner",
    "ProcessSpawner",
    "ProvisionError",
    "RemoteAgentResult",
    "RemoteAgentRunner",
    "RemoteConnection",
    "RemoteResult",
    "RetryScheduled",
    "SSHConfig",
    "SSHConnection",
    "SSHExecutionEnvironment",
    "SecretBundle",
    "SqliteEventEmitter",
    "SqliteVMStateStore",
    "SubprocessSpawner",
    "UbuntuBootstrapper",
    "VMAssignment",
    "VMHandle",
    "VMProvider",
    "VMProvisioned",
    "VMProvisionerProtocol",
    "VMReleased",
    "VMRequirements",
    "VMStateStore",
    "WorkspaceManagerProtocol",
    "WorkspacePath",
    "WorkspaceSpec",
    "WorktreeManager",
]
