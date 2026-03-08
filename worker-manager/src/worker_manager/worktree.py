"""Worktree management: create/validate/remove/registry per PROTOCOL.md Section 8."""

import asyncio
from datetime import UTC, datetime
from pathlib import Path

from worker_manager.ipc import atomic_write
from worker_manager.schemas import WorktreeEntry, WorktreeRegistry

# Module-level lock for registry read-modify-write
_registry_lock = asyncio.Lock()


async def create_worktree(
    project: str,
    issue: int,
    branch: str,
    github_dir: str,
) -> Path:
    """Create a git worktree at ~/github/{project}-wt-{issue} from {branch}.

    The branch must already exist. Raises RuntimeError on failure.
    """
    project_dir = Path(github_dir) / project
    worktree_path = Path(github_dir) / f"{project}-wt-{issue}"

    if worktree_path.exists():
        raise RuntimeError(f"Worktree path already exists: {worktree_path}")

    proc = await asyncio.create_subprocess_exec(
        "git", "worktree", "add", str(worktree_path), branch,
        cwd=str(project_dir),
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )
    stdout, stderr = await proc.communicate()

    if proc.returncode != 0:
        error = stderr.decode().strip() if stderr else stdout.decode().strip()
        raise RuntimeError(f"git worktree add failed: {error}")

    return worktree_path


async def validate_worktree(worktree_path: Path, branch: str) -> None:
    """Validate worktree: correct branch checked out and clean working tree.

    Raises RuntimeError if validation fails.
    """
    if not worktree_path.exists():
        raise RuntimeError(f"Worktree does not exist: {worktree_path}")

    # Check current branch
    proc = await asyncio.create_subprocess_exec(
        "git", "rev-parse", "--abbrev-ref", "HEAD",
        cwd=str(worktree_path),
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )
    stdout, _ = await proc.communicate()
    current_branch = stdout.decode().strip()
    if current_branch != branch:
        raise RuntimeError(
            f"Worktree on wrong branch: expected {branch}, got {current_branch}"
        )

    # Check for clean working tree
    proc = await asyncio.create_subprocess_exec(
        "git", "status", "--porcelain",
        cwd=str(worktree_path),
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )
    stdout, _ = await proc.communicate()
    if stdout.decode().strip():
        raise RuntimeError(f"Worktree has uncommitted changes: {worktree_path}")


async def remove_worktree(worktree_path: Path, project_dir: Path) -> None:
    """Remove a git worktree."""
    proc = await asyncio.create_subprocess_exec(
        "git", "worktree", "remove", str(worktree_path),
        cwd=str(project_dir),
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )
    await proc.communicate()
    # Ignore errors — worktree may already be removed


async def load_registry(registry_path: Path) -> WorktreeRegistry:
    """Load worktree registry from JSON file."""

    def _load() -> WorktreeRegistry:
        if not registry_path.exists():
            return WorktreeRegistry()
        try:
            content = registry_path.read_text()
            return WorktreeRegistry.model_validate_json(content)
        except Exception:
            return WorktreeRegistry()

    return await asyncio.to_thread(_load)


async def save_registry(registry_path: Path, registry: WorktreeRegistry) -> None:
    """Save worktree registry atomically."""
    registry_path.parent.mkdir(parents=True, exist_ok=True)
    await atomic_write(registry_path, registry.model_dump_json(indent=2))


async def check_isolation(
    registry: WorktreeRegistry,
    branch: str,
    worktree_path: Path,
    github_dir: str,
) -> None:
    """Enforce isolation invariants: no shared branches, paths, or main copies.

    Raises RuntimeError on violation.
    """
    wt_str = str(worktree_path)

    for wf_id, entry in registry.worktrees.items():
        if entry.branch == branch:
            raise RuntimeError(
                f"Branch {branch} already in use by workflow {wf_id}"
            )
        if entry.path == wt_str:
            raise RuntimeError(
                f"Worktree path {wt_str} already in use by workflow {wf_id}"
            )

    # Ensure worktree path is not the main working copy
    # Main copy would be ~/github/{project} without -wt- suffix
    if "-wt-" not in worktree_path.name:
        raise RuntimeError(
            f"Worktree path {wt_str} appears to be a main working copy"
        )


async def register_worktree(
    registry_path: Path,
    workflow_id: str,
    project: str,
    issue: int,
    branch: str,
    worktree_path: Path,
    github_dir: str,
) -> None:
    """Register a worktree in the registry with isolation checks. Thread-safe."""
    async with _registry_lock:
        registry = await load_registry(registry_path)
        await check_isolation(registry, branch, worktree_path, github_dir)
        registry.worktrees[workflow_id] = WorktreeEntry(
            project=project,
            issue=issue,
            branch=branch,
            path=str(worktree_path),
            created_at=datetime.now(UTC).isoformat(),
        )
        await save_registry(registry_path, registry)


async def cleanup_worktree(
    workflow_id: str,
    registry_path: Path,
    github_dir: str,
) -> None:
    """Remove worktree and delete registry entry. Called on cleanup dispatch."""
    async with _registry_lock:
        registry = await load_registry(registry_path)
        entry = registry.worktrees.get(workflow_id)
        if entry is None:
            return

        worktree_path = Path(entry.path)
        project_dir = Path(github_dir) / entry.project

        await remove_worktree(worktree_path, project_dir)

        del registry.worktrees[workflow_id]
        await save_registry(registry_path, registry)
