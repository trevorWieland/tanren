"""Adapter interfaces and default implementations.

Protocols define the contract; concrete classes wrap existing module functions.
"""

from worker_manager.adapters.dotenv_provisioner import DotenvEnvProvisioner
from worker_manager.adapters.dotenv_validator import DotenvEnvValidator
from worker_manager.adapters.events import (
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
from worker_manager.adapters.git_postflight import GitPostflightRunner
from worker_manager.adapters.git_preflight import GitPreflightRunner
from worker_manager.adapters.git_workspace import GitAuthConfig, GitWorkspaceManager
from worker_manager.adapters.git_worktree import GitWorktreeManager
from worker_manager.adapters.local_environment import LocalExecutionEnvironment
from worker_manager.adapters.manual_vm import ManualVMProvisioner, NoVMAvailableError
from worker_manager.adapters.null_emitter import NullEventEmitter
from worker_manager.adapters.protocols import (
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
from worker_manager.adapters.protocols import (
    VMProvisioner as VMProvisionerProtocol,
)
from worker_manager.adapters.protocols import (
    WorkspaceManager as WorkspaceManagerProtocol,
)
from worker_manager.adapters.remote_runner import RemoteAgentRunner
from worker_manager.adapters.remote_types import (
    BootstrapResult,
    RemoteAgentResult,
    RemoteResult,
    SecretBundle,
    VMAssignment,
    VMHandle,
    VMRequirements,
    WorkspacePath,
    WorkspaceSpec,
)
from worker_manager.adapters.sqlite_emitter import SqliteEventEmitter
from worker_manager.adapters.sqlite_vm_state import SqliteVMStateStore
from worker_manager.adapters.ssh import SSHConfig, SSHConnection
from worker_manager.adapters.ssh_environment import SSHExecutionEnvironment
from worker_manager.adapters.subprocess_spawner import SubprocessSpawner
from worker_manager.adapters.types import (
    AccessInfo,
    EnvironmentHandle,
    PhaseResult,
    ProvisionError,
)
from worker_manager.adapters.ubuntu_bootstrap import UbuntuBootstrapper

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
    "LocalExecutionEnvironment",
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
