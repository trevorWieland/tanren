"""Git-backed worktree manager adapter."""

from __future__ import annotations

from pathlib import Path

from worker_manager.worktree import (
    cleanup_worktree,
    create_worktree,
    register_worktree,
)


class GitWorktreeManager:
    """Delegates to worktree module functions."""

    async def create(
        self, project: str, issue: int, branch: str, github_dir: str
    ) -> Path:
        return await create_worktree(project, issue, branch, github_dir)

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
        await register_worktree(
            registry_path, workflow_id, project, issue, branch, worktree_path, github_dir
        )

    async def cleanup(
        self, workflow_id: str, registry_path: Path, github_dir: str
    ) -> None:
        await cleanup_worktree(workflow_id, registry_path, github_dir)
