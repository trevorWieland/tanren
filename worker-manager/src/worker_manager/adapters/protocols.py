"""Protocol interfaces for worker manager adapters."""

from __future__ import annotations

from pathlib import Path
from typing import Protocol, runtime_checkable

from worker_manager.adapters.events import Event
from worker_manager.adapters.types import AccessInfo, EnvironmentHandle, PhaseResult
from worker_manager.config import Config
from worker_manager.env.validator import EnvReport
from worker_manager.postflight import PostflightResult
from worker_manager.preflight import PreflightResult
from worker_manager.process import ProcessResult
from worker_manager.schemas import Dispatch


@runtime_checkable
class WorktreeManager(Protocol):
    """Create, register, and clean up git worktrees.

    Called during setup and cleanup phases only — not during work phases.
    create() makes a new worktree from the project's default branch,
    register() records it in the worktree registry for isolation enforcement,
    and cleanup() removes both the worktree and registry entry.

    Default implementation: GitWorktreeManager (git worktree add/remove).
    """

    async def create(self, project: str, issue: int, branch: str, github_dir: str) -> Path: ...

    async def register(
        self,
        registry_path: Path,
        workflow_id: str,
        project: str,
        issue: int,
        branch: str,
        worktree_path: Path,
        github_dir: str,
    ) -> None: ...

    async def cleanup(self, workflow_id: str, registry_path: Path, github_dir: str) -> None: ...


@runtime_checkable
class PreflightRunner(Protocol):
    """Run pre-flight checks before agent process spawn.

    Validates worktree state (correct branch, clean status), takes file
    snapshots for post-flight integrity comparison, and repairs minor
    issues (e.g. uncommitted .agent-status files).

    Default implementation: GitPreflightRunner.
    """

    async def run(
        self, worktree_path: Path, branch: str, spec_folder: Path, phase: str
    ) -> PreflightResult: ...


@runtime_checkable
class PostflightRunner(Protocol):
    """Run post-flight integrity checks after agent process exits.

    Compares file hashes against preflight snapshots, reverts unauthorized
    spec.md modifications, and pushes committed changes to the remote.
    Skips push on error/timeout outcomes to avoid pushing broken state.

    Default implementation: GitPostflightRunner.
    """

    async def run(
        self,
        worktree_path: Path,
        branch: str,
        phase: str,
        preflight_hashes: dict[str, str],
        preflight_backups: dict[str, str],
        *,
        skip_push: bool = False,
    ) -> PostflightResult: ...


@runtime_checkable
class ProcessSpawner(Protocol):
    """Spawn CLI processes for dispatched work.

    Routes to the appropriate CLI based on dispatch.cli (opencode, codex,
    claude, bash). Handles prompt assembly, temp file management, timeout
    enforcement (SIGTERM → 5s grace → SIGKILL), and process group isolation.

    Default implementation: SubprocessSpawner.
    """

    async def spawn(
        self,
        dispatch: Dispatch,
        worktree_path: Path,
        config: Config,
        *,
        task_env: dict[str, str] | None = None,
    ) -> ProcessResult: ...


@runtime_checkable
class EnvValidator(Protocol):
    """Validate env requirements before work phases.

    Loads env vars from tanren.yml declarations, .env files, and secrets
    stores. Returns an EnvReport (pass/fail with actionable diagnostics)
    and a dict of validated env vars to inject into the agent process.

    Default implementation: DotenvEnvValidator.
    """

    async def load_and_validate(self, project_root: Path) -> tuple[EnvReport, dict[str, str]]: ...


@runtime_checkable
class EnvProvisioner(Protocol):
    """Provision .env files in worktrees during setup.

    Sync method — caller wraps in asyncio.to_thread().
    """

    def provision(self, worktree_path: Path, project_dir: Path) -> int: ...


@runtime_checkable
class EventEmitter(Protocol):
    """Emit structured events for observability.

    Events include DispatchReceived, PhaseStarted, PhaseCompleted,
    PreflightCompleted, PostflightCompleted, ErrorOccurred, and
    RetryScheduled. Used for metering, auditing, and dashboard display.

    Default implementations: NullEventEmitter (no-op), SqliteEventEmitter.
    """

    async def emit(self, event: Event) -> None: ...
    async def close(self) -> None: ...


@runtime_checkable
class ExecutionEnvironment(Protocol):
    """Where work runs. Local subprocess, Docker container, remote VM.

    Lifecycle: provision() → execute() → get_access_info() → teardown().
    The manager calls provision() to validate the environment and prepare it,
    execute() to run the agent process with retry logic, and teardown() to
    clean up. get_access_info() provides debug connection details.
    """

    async def provision(self, dispatch: Dispatch, config: Config) -> EnvironmentHandle: ...

    async def execute(
        self,
        handle: EnvironmentHandle,
        dispatch: Dispatch,
        config: Config,
        *,
        dispatch_stem: str = "",
    ) -> PhaseResult: ...

    async def get_access_info(self, handle: EnvironmentHandle) -> AccessInfo: ...

    async def teardown(self, handle: EnvironmentHandle) -> None: ...
