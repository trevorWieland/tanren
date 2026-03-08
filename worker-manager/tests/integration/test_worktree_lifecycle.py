"""Integration test: worktree lifecycle with real git repo."""

from pathlib import Path

import pytest

from worker_manager.worktree import (
    cleanup_worktree,
    create_worktree,
    load_registry,
    register_worktree,
    remove_worktree,
    validate_worktree,
)


async def _setup_git_repo(tmp_path: Path) -> tuple[Path, str]:
    """Create a bare git repo with a feature branch for testing."""
    import asyncio

    repo = tmp_path / "test-project"
    repo.mkdir()

    # Init repo with initial commit
    for cmd in [
        ["git", "init"],
        ["git", "config", "user.email", "test@test.com"],
        ["git", "config", "user.name", "Test"],
        ["git", "checkout", "-b", "main"],
    ]:
        proc = await asyncio.create_subprocess_exec(
            *cmd, cwd=str(repo),
            stdout=asyncio.subprocess.DEVNULL,
            stderr=asyncio.subprocess.DEVNULL,
        )
        await proc.wait()

    # Create initial file and commit
    (repo / "README.md").write_text("# Test")
    for cmd in [
        ["git", "add", "."],
        ["git", "commit", "-m", "initial"],
    ]:
        proc = await asyncio.create_subprocess_exec(
            *cmd, cwd=str(repo),
            stdout=asyncio.subprocess.DEVNULL,
            stderr=asyncio.subprocess.DEVNULL,
        )
        await proc.wait()

    # Create feature branch
    branch = "feature-123"
    proc = await asyncio.create_subprocess_exec(
        "git", "branch", branch,
        cwd=str(repo),
        stdout=asyncio.subprocess.DEVNULL,
        stderr=asyncio.subprocess.DEVNULL,
    )
    await proc.wait()

    return repo, branch


class TestWorktreeLifecycle:
    @pytest.mark.asyncio
    async def test_create_validate_remove(self, tmp_path: Path):
        """Full lifecycle: create -> validate -> remove."""
        github_dir = tmp_path
        repo, branch = await _setup_git_repo(tmp_path)

        # Create worktree
        wt_path = await create_worktree("test-project", 123, branch, str(github_dir))
        assert wt_path.exists()
        assert wt_path.name == "test-project-wt-123"

        # Validate
        await validate_worktree(wt_path, branch)

        # Remove
        await remove_worktree(wt_path, repo)
        assert not wt_path.exists()

    @pytest.mark.asyncio
    async def test_validate_wrong_branch(self, tmp_path: Path):
        """Validation should fail if wrong branch."""
        github_dir = tmp_path
        repo, branch = await _setup_git_repo(tmp_path)
        wt_path = await create_worktree("test-project", 123, branch, str(github_dir))

        with pytest.raises(RuntimeError, match="wrong branch"):
            await validate_worktree(wt_path, "nonexistent-branch")

        await remove_worktree(wt_path, repo)

    @pytest.mark.asyncio
    async def test_create_duplicate_fails(self, tmp_path: Path):
        """Creating a worktree at an existing path should fail."""
        github_dir = tmp_path
        repo, branch = await _setup_git_repo(tmp_path)
        await create_worktree("test-project", 123, branch, str(github_dir))

        with pytest.raises(RuntimeError, match="already exists"):
            await create_worktree("test-project", 123, branch, str(github_dir))

        # Cleanup
        wt_path = github_dir / "test-project-wt-123"
        await remove_worktree(wt_path, repo)

    @pytest.mark.asyncio
    async def test_register_and_cleanup(self, tmp_path: Path):
        """Register a worktree, then clean it up via cleanup_worktree."""
        github_dir = tmp_path
        _repo, branch = await _setup_git_repo(tmp_path)
        wt_path = await create_worktree("test-project", 123, branch, str(github_dir))

        registry_path = tmp_path / "worktrees.json"
        workflow_id = "wf-test-project-123-1000"

        await register_worktree(
            registry_path, workflow_id, "test-project", 123, branch, wt_path, str(github_dir)
        )

        # Verify registered
        reg = await load_registry(registry_path)
        assert workflow_id in reg.worktrees

        # Cleanup
        await cleanup_worktree(workflow_id, registry_path, str(github_dir))

        # Verify cleaned up
        reg = await load_registry(registry_path)
        assert workflow_id not in reg.worktrees
        assert not wt_path.exists()
