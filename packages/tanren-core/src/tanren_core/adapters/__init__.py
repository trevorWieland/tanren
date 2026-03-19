"""Adapter interfaces and default implementations.

Protocols define the contract; concrete classes wrap existing module functions.
"""

from tanren_core.adapters.credentials import (
    CLI_CREDENTIAL_PROVIDERS,
    DEFAULT_CREDENTIAL_PROVIDERS,
    ClaudeCredentialProvider,
    CodexCredentialProvider,
    CredentialProvider,
    OpencodeCredentialProvider,
    all_credential_cleanup_paths,
    inject_all_cli_credentials,
    providers_for_clis,
)
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
    TokenUsageRecorded,
    VMProvisioned,
    VMReleased,
)
from tanren_core.adapters.git_postflight import GitPostflightRunner
from tanren_core.adapters.git_preflight import GitPreflightRunner
from tanren_core.adapters.git_workspace import GitAuthConfig, GitWorkspaceManager
from tanren_core.adapters.git_worktree import GitWorktreeManager

try:  # noqa: SIM105, RUF067
    from tanren_core.adapters.gcp_vm import GCPProvisionerSettings, GCPVMProvisioner
except ImportError:  # google-cloud-compute not installed
    pass
try:  # noqa: SIM105, RUF067
    from tanren_core.adapters.hetzner_vm import HetznerProvisionerSettings, HetznerVMProvisioner
except ImportError:  # hcloud not installed
    pass
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
    "CLI_CREDENTIAL_PROVIDERS",
    "DEFAULT_CREDENTIAL_PROVIDERS",
    "AccessInfo",
    "BootstrapCompleted",
    "BootstrapResult",
    "ClaudeCredentialProvider",
    "CodexCredentialProvider",
    "CredentialProvider",
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
    "GCPProvisionerSettings",
    "GCPVMProvisioner",
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
    "OpencodeCredentialProvider",
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
    "TokenUsageRecorded",
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
    "all_credential_cleanup_paths",
    "inject_all_cli_credentials",
    "providers_for_clis",
]
