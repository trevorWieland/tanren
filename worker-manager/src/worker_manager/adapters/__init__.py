"""Adapter interfaces and default implementations.

Protocols define the contract; concrete classes wrap existing module functions.
"""

from worker_manager.adapters.dotenv_provisioner import DotenvEnvProvisioner
from worker_manager.adapters.dotenv_validator import DotenvEnvValidator
from worker_manager.adapters.events import (
    DispatchReceived,
    ErrorOccurred,
    Event,
    PhaseCompleted,
    PhaseStarted,
    PostflightCompleted,
    PreflightCompleted,
    RetryScheduled,
)
from worker_manager.adapters.git_postflight import GitPostflightRunner
from worker_manager.adapters.git_preflight import GitPreflightRunner
from worker_manager.adapters.git_worktree import GitWorktreeManager
from worker_manager.adapters.local_environment import LocalExecutionEnvironment
from worker_manager.adapters.null_emitter import NullEventEmitter
from worker_manager.adapters.protocols import (
    EnvProvisioner,
    EnvValidator,
    EventEmitter,
    ExecutionEnvironment,
    PostflightRunner,
    PreflightRunner,
    ProcessSpawner,
    WorktreeManager,
)
from worker_manager.adapters.sqlite_emitter import SqliteEventEmitter
from worker_manager.adapters.subprocess_spawner import SubprocessSpawner
from worker_manager.adapters.types import (
    AccessInfo,
    EnvironmentHandle,
    PhaseResult,
    ProvisionError,
)

__all__ = [
    "AccessInfo",
    "DispatchReceived",
    "DotenvEnvProvisioner",
    "DotenvEnvValidator",
    "EnvProvisioner",
    "EnvValidator",
    "EnvironmentHandle",
    "ErrorOccurred",
    "Event",
    "EventEmitter",
    "ExecutionEnvironment",
    "GitPostflightRunner",
    "GitPreflightRunner",
    "GitWorktreeManager",
    "LocalExecutionEnvironment",
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
    "RetryScheduled",
    "SqliteEventEmitter",
    "SubprocessSpawner",
    "WorktreeManager",
]
