"""Git-backed worktree manager adapter."""

from __future__ import annotations

from typing import TYPE_CHECKING

from tanren_core.worktree import (
    cleanup_worktree,
    create_worktree,
    register_worktree,
)

if TYPE_CHECKING:
    from pathlib import Path


class GitWorktreeManager:
    """Delegates to worktree module functions."""

    async def create(self, project: str, issue: str, branch: str, github_dir: str) -> Path:
        """Create a new git worktree for the given project and issue.

        Returns:
            Path to the created worktree directory.
        """
        return await create_worktree(project, issue, branch, github_dir)

    async def register(
        self,
        registry_path: Path,
        workflow_id: str,
        project: str,
        issue: str,
        branch: str,
        worktree_path: Path,
        github_dir: str,
    ) -> None:
        """Register a worktree in the worktree registry for isolation enforcement."""
        await register_worktree(
            registry_path, workflow_id, project, issue, branch, worktree_path, github_dir
        )

    async def cleanup(self, workflow_id: str, registry_path: Path, github_dir: str) -> None:
        """Remove the worktree and its registry entry."""
        await cleanup_worktree(workflow_id, registry_path, github_dir)
