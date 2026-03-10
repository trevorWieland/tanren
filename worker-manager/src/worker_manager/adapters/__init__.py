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
from worker_manager.adapters.null_emitter import NullEventEmitter
from worker_manager.adapters.protocols import (
    EnvProvisioner,
    EnvValidator,
    EventEmitter,
    PostflightRunner,
    PreflightRunner,
    ProcessSpawner,
    WorktreeManager,
)
from worker_manager.adapters.sqlite_emitter import SqliteEventEmitter
from worker_manager.adapters.subprocess_spawner import SubprocessSpawner

__all__ = [
    "DispatchReceived",
    "DotenvEnvProvisioner",
    "DotenvEnvValidator",
    "EnvProvisioner",
    "EnvValidator",
    "ErrorOccurred",
    "Event",
    "EventEmitter",
    "GitPostflightRunner",
    "GitPreflightRunner",
    "GitWorktreeManager",
    "NullEventEmitter",
    "PhaseCompleted",
    "PhaseStarted",
    "PostflightCompleted",
    "PostflightRunner",
    "PreflightCompleted",
    "PreflightRunner",
    "ProcessSpawner",
    "RetryScheduled",
    "SqliteEventEmitter",
    "SubprocessSpawner",
    "WorktreeManager",
]
