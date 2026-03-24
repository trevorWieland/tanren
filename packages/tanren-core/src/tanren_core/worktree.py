"""Worktree management: create/validate/remove/registry."""

import asyncio
import logging
import os
import shutil
from datetime import UTC, datetime
from pathlib import Path

from tanren_core.schemas import WorktreeEntry, WorktreeRegistry


async def atomic_write(path: Path, content: str) -> None:
    """Write content atomically: write .tmp, fsync, rename."""

    def _write() -> None:
        tmp_path = path.with_suffix(".tmp")
        with open(tmp_path, "w") as f:
            f.write(content)
            f.flush()
            os.fsync(f.fileno())
        tmp_path.rename(path)

    await asyncio.to_thread(_write)


logger = logging.getLogger(__name__)

# Module-level lock for registry read-modify-write
_registry_lock = asyncio.Lock()


async def get_default_branch(project_dir: Path) -> str:
    """Detect the default branch (main/master) for a repository.

    Tries symbolic-ref first, then falls back to checking main/master.

    Returns:
        The default branch name.

    Raises:
        RuntimeError: If no default branch can be determined.
    """
    # Try symbolic-ref for origin HEAD
    proc = await asyncio.create_subprocess_exec(
        "git",
        "symbolic-ref",
        "refs/remotes/origin/HEAD",
        cwd=str(project_dir),
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )
    stdout, _ = await proc.communicate()
    if proc.returncode == 0:
        ref = stdout.decode().strip()
        return ref.removeprefix("refs/remotes/origin/")

    # Fallback: check main then master
    for candidate in ("main", "master"):
        proc = await asyncio.create_subprocess_exec(
            "git",
            "rev-parse",
            "--verify",
            candidate,
            cwd=str(project_dir),
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        await proc.communicate()
        if proc.returncode == 0:
            return candidate

    raise RuntimeError(f"Cannot determine default branch in {project_dir}")


async def _is_tracked_worktree(project_dir: Path, worktree_path: Path) -> bool:
    """Check if a path is tracked by git as a worktree.

    Returns:
        True if the path is a tracked worktree.
    """
    proc = await asyncio.create_subprocess_exec(
        "git",
        "worktree",
        "list",
        "--porcelain",
        cwd=str(project_dir),
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )
    stdout, _ = await proc.communicate()
    if proc.returncode != 0:
        return False

    resolved = str(worktree_path.resolve())  # noqa: ASYNC240 — in-memory path resolution
    for line in stdout.decode().splitlines():
        if line.startswith("worktree ") and line[9:] == resolved:
            return True
    return False


async def _get_worktree_branch(worktree_path: Path) -> str:
    """Get the current branch of a worktree directory.

    Returns:
        Branch name string.
    """
    proc = await asyncio.create_subprocess_exec(
        "git",
        "rev-parse",
        "--abbrev-ref",
        "HEAD",
        cwd=str(worktree_path),
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )
    stdout, _ = await proc.communicate()
    return stdout.decode().strip()


async def create_worktree(
    project: str,
    issue: str,
    branch: str,
    github_dir: str,
) -> Path:
    """Create a git worktree at ~/github/{project}-wt-{issue} from {branch}.

    The branch must already exist. If the main repo currently has the target
    branch checked out, switches the main repo to the default branch first.
    Handles stale directories and idempotent re-creation gracefully.

    Returns:
        Path to the created or existing worktree directory.

    Raises:
        RuntimeError: If the worktree already exists on a different branch,
            the main repo cannot be switched, or ``git worktree add`` fails.
    """
    project_dir = Path(github_dir) / project
    worktree_path = Path(github_dir) / f"{project}-wt-{issue}"

    if worktree_path.exists():
        if await _is_tracked_worktree(project_dir, worktree_path):
            # Git still tracks this worktree — check branch
            current = await _get_worktree_branch(worktree_path)
            if current == branch:
                logger.info("Worktree already exists on correct branch: %s", worktree_path)
                return worktree_path
            raise RuntimeError(
                f"Worktree {worktree_path} exists on branch {current}, expected {branch}"
            )
        # Stale directory — git doesn't track it
        logger.warning("Removing stale worktree directory: %s", worktree_path)
        shutil.rmtree(worktree_path)
        proc = await asyncio.create_subprocess_exec(
            "git",
            "worktree",
            "prune",
            cwd=str(project_dir),
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        await proc.communicate()

    # If main repo has the target branch checked out, switch away first
    proc = await asyncio.create_subprocess_exec(
        "git",
        "branch",
        "--show-current",
        cwd=str(project_dir),
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )
    stdout, _ = await proc.communicate()
    current_branch = stdout.decode().strip()

    if current_branch == branch:
        default_branch = await get_default_branch(project_dir)
        proc = await asyncio.create_subprocess_exec(
            "git",
            "checkout",
            default_branch,
            cwd=str(project_dir),
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        _, stderr = await proc.communicate()
        if proc.returncode != 0:
            error = stderr.decode().strip()
            raise RuntimeError(
                f"Cannot switch main repo from {branch} to {default_branch}: {error}"
            )

    proc = await asyncio.create_subprocess_exec(
        "git",
        "worktree",
        "add",
        str(worktree_path),
        branch,
        cwd=str(project_dir),
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )
    stdout, stderr = await proc.communicate()

    if proc.returncode != 0:
        error = stderr.decode().strip() if stderr else stdout.decode().strip()
        raise RuntimeError(f"git worktree add failed: {error}")

    return worktree_path


async def remove_worktree(worktree_path: Path, project_dir: Path) -> None:
    """Remove a git worktree. Falls back to rmtree + prune if git remove fails."""
    proc = await asyncio.create_subprocess_exec(
        "git",
        "worktree",
        "remove",
        "--force",
        str(worktree_path),
        cwd=str(project_dir),
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )
    _, stderr = await proc.communicate()
    if proc.returncode != 0:
        logger.warning("git worktree remove failed: %s", stderr.decode().strip())

    # If directory still exists, force-clean it
    if worktree_path.exists():  # noqa: ASYNC240 — quick existence check after git command
        logger.warning("Worktree directory persisted, removing via rmtree: %s", worktree_path)
        shutil.rmtree(worktree_path, ignore_errors=True)
        proc = await asyncio.create_subprocess_exec(
            "git",
            "worktree",
            "prune",
            cwd=str(project_dir),
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        await proc.communicate()


async def load_registry(registry_path: Path) -> WorktreeRegistry:
    """Load worktree registry from JSON file.

    Returns:
        WorktreeRegistry loaded from file, or empty default.
    """

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


async def check_isolation(  # noqa: RUF029 — kept async for consistency with callers
    registry: WorktreeRegistry,
    workflow_id: str,
    branch: str,
    worktree_path: Path,
    github_dir: str,  # noqa: ARG001 — required by interface
) -> None:
    """Enforce isolation invariants: no shared branches, paths, or main copies.

    Skips entries for the same workflow_id (allows re-registration on resume).

    Raises:
        RuntimeError: If a branch or path conflict is detected, or the path
            appears to be a main working copy.
    """
    wt_str = str(worktree_path)

    for wf_id, entry in registry.worktrees.items():
        if wf_id == workflow_id:
            continue
        if entry.branch == branch:
            raise RuntimeError(f"Branch {branch} already in use by workflow {wf_id}")
        if entry.path == wt_str:
            raise RuntimeError(f"Worktree path {wt_str} already in use by workflow {wf_id}")

    # Ensure worktree path is not the main working copy
    # Main copy would be ~/github/{project} without -wt- suffix
    if "-wt-" not in worktree_path.name:
        raise RuntimeError(f"Worktree path {wt_str} appears to be a main working copy")


async def register_worktree(
    registry_path: Path,
    workflow_id: str,
    project: str,
    issue: str,
    branch: str,
    worktree_path: Path,
    github_dir: str,
) -> None:
    """Register a worktree in the registry with isolation checks. Thread-safe."""
    async with _registry_lock:
        registry = await load_registry(registry_path)
        await check_isolation(registry, workflow_id, branch, worktree_path, github_dir)
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
            logger.info("No registry entry for %s, cleanup is a no-op", workflow_id)
            return

        worktree_path = Path(entry.path)
        project_dir = Path(github_dir) / entry.project

        try:
            await remove_worktree(worktree_path, project_dir)
        except Exception:
            logger.warning(
                "remove_worktree failed for %s, clearing registry entry anyway",
                workflow_id,
                exc_info=True,
            )

        # Always clear the registry entry — even if filesystem cleanup
        # was incomplete.  A stale registry entry blocks all future
        # dispatches on the same branch; a stale directory is harmless
        # (git worktree add handles it idempotently).
        del registry.worktrees[workflow_id]
        await save_registry(registry_path, registry)
