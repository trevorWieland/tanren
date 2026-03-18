"""Protocol interfaces for worker manager adapters.

Reference docs:
- worker-manager/ADAPTERS.md
- docs/architecture/overview.md
"""

from __future__ import annotations

from pathlib import Path
from typing import Protocol, runtime_checkable

from tanren_core.adapters.events import Event
from tanren_core.adapters.remote_types import (
    BootstrapResult,
    RemoteResult,
    SecretBundle,
    VMAssignment,
    VMHandle,
    VMRequirements,
    WorkspacePath,
    WorkspaceSpec,
)
from tanren_core.adapters.types import AccessInfo, EnvironmentHandle, PhaseResult
from tanren_core.config import Config
from tanren_core.env.validator import EnvReport
from tanren_core.postflight import PostflightResult
from tanren_core.preflight import PreflightResult
from tanren_core.process import ProcessResult
from tanren_core.schemas import Dispatch


@runtime_checkable
class WorktreeManager(Protocol):
    """Create, register, and clean up git worktrees.

    Called during setup and cleanup phases only — not during work phases.
    create() makes a new worktree from the project's default branch,
    register() records it in the worktree registry for isolation enforcement,
    and cleanup() removes both the worktree and registry entry.

    Default implementation: GitWorktreeManager (git worktree add/remove).
    """

    async def create(self, project: str, issue: int, branch: str, github_dir: str) -> Path:
        """Create a new git worktree for the given project and issue."""
        ...

    async def register(
        self,
        registry_path: Path,
        workflow_id: str,
        project: str,
        issue: int,
        branch: str,
        worktree_path: Path,
        github_dir: str,
    ) -> None:
        """Register a worktree in the registry for isolation enforcement."""
        ...

    async def cleanup(self, workflow_id: str, registry_path: Path, github_dir: str) -> None:
        """Remove a worktree and its registry entry."""
        ...


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
    ) -> PreflightResult:
        """Run pre-flight checks and return the result."""
        ...


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
    ) -> PostflightResult:
        """Run post-flight integrity checks and optionally push changes."""
        ...


@runtime_checkable
class ProcessSpawner(Protocol):
    """Spawn CLI processes for dispatched work.

    Routes to the appropriate CLI based on dispatch.cli (opencode, codex,
    claude, bash). Handles prompt assembly, temp file management, timeout
    enforcement (SIGTERM -> 5s grace -> SIGKILL), and process group isolation.

    Default implementation: SubprocessSpawner.
    """

    async def spawn(
        self,
        dispatch: Dispatch,
        worktree_path: Path,
        config: Config,
        *,
        task_env: dict[str, str] | None = None,
    ) -> ProcessResult:
        """Spawn a CLI process for the given dispatch and return the result."""
        ...


@runtime_checkable
class EnvValidator(Protocol):
    """Validate env requirements before work phases.

    Loads env vars from tanren.yml declarations, .env files, and secrets
    stores. Returns an EnvReport (pass/fail with actionable diagnostics)
    and a dict of validated env vars to inject into the agent process.

    Default implementation: DotenvEnvValidator.
    """

    async def load_and_validate(self, project_root: Path) -> tuple[EnvReport, dict[str, str]]:
        """Load and validate environment variables for the project."""
        ...


@runtime_checkable
class EnvProvisioner(Protocol):
    """Provision .env files in worktrees during setup.

    Sync method — caller wraps in asyncio.to_thread().
    """

    def provision(self, worktree_path: Path, project_dir: Path) -> int:
        """Provision .env files and return the count of vars provisioned."""
        ...


@runtime_checkable
class EventEmitter(Protocol):
    """Emit structured events for observability.

    Events include DispatchReceived, PhaseStarted, PhaseCompleted,
    PreflightCompleted, PostflightCompleted, ErrorOccurred, and
    RetryScheduled. Used for metering, auditing, and dashboard display.

    Default implementations: NullEventEmitter (no-op), SqliteEventEmitter.
    """

    async def emit(self, event: Event) -> None:
        """Emit a structured event."""
        ...

    async def close(self) -> None:
        """Close any resources held by the emitter."""
        ...


@runtime_checkable
class ExecutionEnvironment(Protocol):
    """Where work runs. Local subprocess, Docker container, remote VM.

    Lifecycle: provision() -> execute() -> get_access_info() -> teardown().
    The manager calls provision() to validate the environment and prepare it,
    execute() to run the agent process with retry logic, and teardown() to
    clean up. get_access_info() provides debug connection details.
    """

    async def provision(self, dispatch: Dispatch, config: Config) -> EnvironmentHandle:
        """Validate and prepare the execution environment."""
        ...

    async def execute(
        self,
        handle: EnvironmentHandle,
        dispatch: Dispatch,
        config: Config,
        *,
        dispatch_stem: str = "",
    ) -> PhaseResult:
        """Run the agent process with retry logic and return the result."""
        ...

    async def get_access_info(self, handle: EnvironmentHandle) -> AccessInfo:
        """Return connection info for debugging a running environment."""
        ...

    async def teardown(self, handle: EnvironmentHandle) -> None:
        """Clean up the execution environment and release resources."""
        ...

    async def release_vm(self, vm_handle: VMHandle) -> None:
        """Release a VM through the underlying provider.

        Does not update state tracking — the caller is responsible for
        calling VMStateStore.record_release() separately.
        """
        ...


@runtime_checkable
class VMProvisioner(Protocol):
    """Provision and release VMs.

    Manages a pool of VMs — acquires one matching the requirements
    and releases it back when done.
    """

    async def acquire(self, requirements: VMRequirements) -> VMHandle:
        """Acquire a VM matching the given requirements."""
        ...

    async def release(self, handle: VMHandle) -> None:
        """Release a VM back to the pool."""
        ...

    async def list_active(self) -> list[VMHandle]:
        """List all currently active VMs."""
        ...


@runtime_checkable
class EnvironmentBootstrapper(Protocol):
    """Bootstrap a VM with required development tools.

    Idempotent — checks for existing installations before running
    install commands. Writes a marker file on completion.
    """

    async def bootstrap(self, conn: RemoteConnection, *, force: bool = False) -> BootstrapResult:
        """Install development tools on the remote VM."""
        ...

    async def is_bootstrapped(self, conn: RemoteConnection) -> bool:
        """Check whether the VM has already been bootstrapped."""
        ...


@runtime_checkable
class WorkspaceManager(Protocol):
    """Manage remote workspaces — clone, secrets, cleanup.

    Note: This is distinct from WorktreeManager which handles local git worktrees.
    """

    async def setup(self, conn: RemoteConnection, spec: WorkspaceSpec) -> WorkspacePath:
        """Clone or pull the repo and run setup commands."""
        ...

    async def inject_secrets(
        self,
        conn: RemoteConnection,
        workspace: WorkspacePath,
        secrets: SecretBundle,
    ) -> None:
        """Write secret files into the remote workspace."""
        ...

    def push_command(self, workspace_path: str, branch: str) -> str:
        """Return an auth-prefixed git push command string."""
        ...

    async def cleanup(self, conn: RemoteConnection, workspace: WorkspacePath) -> None:
        """Remove secret files and auth helpers from the workspace."""
        ...


@runtime_checkable
class RemoteConnection(Protocol):
    """Execute commands and transfer files on a remote host.

    All operations are async. download_content returns None for
    missing files (agent-proof — the agent may delete signal files).
    """

    async def run(
        self,
        command: str,
        *,
        timeout: int | None = None,  # noqa: ASYNC109 — protocol signature, not an actual timeout
        stdin_data: str | None = None,
        request_pty: bool = False,
    ) -> RemoteResult:
        """Execute a command on the remote host."""
        ...

    async def upload_content(self, content: str, remote_path: str) -> None:
        """Upload string content to a remote file."""
        ...

    async def download_content(self, remote_path: str) -> str | None:
        """Download content from a remote file, returning None if missing."""
        ...

    async def check_connection(self) -> bool:
        """Test connectivity to the remote host."""
        ...

    def get_host_identifier(self) -> str:
        """Return a human-readable host identifier."""
        ...


@runtime_checkable
class VMStateStore(Protocol):
    """Persist VM assignment state for startup recovery.

    Records which VMs are assigned to which workflows. On startup,
    the manager can check active assignments and release unreachable VMs.
    """

    async def record_assignment(
        self, vm_id: str, workflow_id: str, project: str, spec: str, host: str
    ) -> None:
        """Record a new VM assignment."""
        ...

    async def record_release(self, vm_id: str) -> None:
        """Mark a VM assignment as released."""
        ...

    async def get_active_assignments(self) -> list[VMAssignment]:
        """Get all currently active (unreleased) VM assignments."""
        ...

    async def get_assignment(self, vm_id: str) -> VMAssignment | None:
        """Get a specific VM assignment by ID."""
        ...

    async def close(self) -> None:
        """Close any resources held by the state store."""
        ...
