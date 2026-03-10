"""Protocol interfaces for worker manager adapters."""

from __future__ import annotations

from pathlib import Path
from typing import Protocol, runtime_checkable

from worker_manager.adapters.events import Event
from worker_manager.config import Config
from worker_manager.env.validator import EnvReport
from worker_manager.postflight import PostflightResult
from worker_manager.preflight import PreflightResult
from worker_manager.process import ProcessResult
from worker_manager.schemas import Dispatch


@runtime_checkable
class WorktreeManager(Protocol):
    """Create, register, and clean up git worktrees."""

    async def create(
        self, project: str, issue: int, branch: str, github_dir: str
    ) -> Path: ...

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

    async def cleanup(
        self, workflow_id: str, registry_path: Path, github_dir: str
    ) -> None: ...


@runtime_checkable
class PreflightRunner(Protocol):
    """Run pre-flight checks before agent process spawn."""

    async def run(
        self, worktree_path: Path, branch: str, spec_folder: Path, phase: str
    ) -> PreflightResult: ...


@runtime_checkable
class PostflightRunner(Protocol):
    """Run post-flight integrity checks after agent process exits."""

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
    """Spawn CLI processes for dispatched work."""

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
    """Validate env requirements before work phases."""

    async def load_and_validate(
        self, project_root: Path
    ) -> tuple[EnvReport, dict[str, str]]: ...


@runtime_checkable
class EnvProvisioner(Protocol):
    """Provision .env files in worktrees during setup.

    Sync method — caller wraps in asyncio.to_thread().
    """

    def provision(self, worktree_path: Path, project_dir: Path) -> int: ...


@runtime_checkable
class EventEmitter(Protocol):
    """Emit structured events for observability."""

    async def emit(self, event: Event) -> None: ...
    async def close(self) -> None: ...
