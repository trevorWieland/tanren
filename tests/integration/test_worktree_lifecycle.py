"""Integration test: worktree lifecycle with real git repo."""

import asyncio
import shutil
from pathlib import Path

import pytest

from tanren_core.schemas import WorktreeEntry, WorktreeRegistry
from tanren_core.worktree import (
    check_isolation,
    cleanup_worktree,
    create_worktree,
    load_registry,
    register_worktree,
    remove_worktree,
    save_registry,
)


async def _setup_git_repo(tmp_path: Path) -> tuple[Path, str]:
    """Create a bare git repo with a feature branch for testing."""
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
            *cmd,
            cwd=str(repo),
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
            *cmd,
            cwd=str(repo),
            stdout=asyncio.subprocess.DEVNULL,
            stderr=asyncio.subprocess.DEVNULL,
        )
        await proc.wait()

    # Create feature branch
    branch = "feature-123"
    proc = await asyncio.create_subprocess_exec(
        "git",
        "branch",
        branch,
        cwd=str(repo),
        stdout=asyncio.subprocess.DEVNULL,
        stderr=asyncio.subprocess.DEVNULL,
    )
    await proc.wait()

    return repo, branch


class TestWorktreeBranchSwitch:
    @pytest.mark.asyncio
    async def test_create_worktree_when_branch_checked_out(self, tmp_path: Path):
        """Should auto-switch main repo to default branch when target branch is checked out."""
        github_dir = tmp_path
        repo, branch = await _setup_git_repo(tmp_path)

        # Checkout the feature branch in the main repo
        proc = await asyncio.create_subprocess_exec(
            "git",
            "checkout",
            branch,
            cwd=str(repo),
            stdout=asyncio.subprocess.DEVNULL,
            stderr=asyncio.subprocess.DEVNULL,
        )
        await proc.wait()

        # Verify we're on the feature branch
        proc = await asyncio.create_subprocess_exec(
            "git",
            "branch",
            "--show-current",
            cwd=str(repo),
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        stdout, _ = await proc.communicate()
        assert stdout.decode().strip() == branch

        # create_worktree should succeed by switching main repo to default branch
        wt_path = await create_worktree("test-project", "123", branch, str(github_dir))
        assert wt_path.exists()

        # Verify main repo switched to default branch
        proc = await asyncio.create_subprocess_exec(
            "git",
            "branch",
            "--show-current",
            cwd=str(repo),
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        stdout, _ = await proc.communicate()
        assert stdout.decode().strip() == "main"

        await remove_worktree(wt_path, repo)


class TestWorktreeLifecycle:
    @pytest.mark.asyncio
    async def test_create_and_remove(self, tmp_path: Path):
        """Full lifecycle: create -> remove."""
        github_dir = tmp_path
        repo, branch = await _setup_git_repo(tmp_path)

        # Create worktree
        wt_path = await create_worktree("test-project", "123", branch, str(github_dir))
        assert wt_path.exists()
        assert wt_path.name == "test-project-wt-123"

        # Remove
        await remove_worktree(wt_path, repo)
        assert not wt_path.exists()

    @pytest.mark.asyncio
    async def test_register_and_cleanup(self, tmp_path: Path):
        """Register a worktree, then clean it up via cleanup_worktree."""
        github_dir = tmp_path
        _repo, branch = await _setup_git_repo(tmp_path)
        wt_path = await create_worktree("test-project", "123", branch, str(github_dir))

        registry_path = tmp_path / "worktrees.json"
        workflow_id = "wf-test-project-123-1000"

        await register_worktree(
            registry_path, workflow_id, "test-project", "123", branch, wt_path, str(github_dir)
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


class TestWorktreeHyphenatedIssue:
    @pytest.mark.asyncio
    async def test_create_register_cleanup_with_hyphenated_issue(self, tmp_path: Path):
        """Full lifecycle with a Linear-style hyphenated issue ID (PROJ-123)."""
        github_dir = tmp_path
        _repo, branch = await _setup_git_repo(tmp_path)

        issue = "PROJ-123"
        wt_path = await create_worktree("test-project", issue, branch, str(github_dir))
        assert wt_path.exists()
        assert wt_path.name == "test-project-wt-PROJ-123"

        registry_path = tmp_path / "worktrees.json"
        workflow_id = "wf-test-project-PROJ-123-1000"

        await register_worktree(
            registry_path, workflow_id, "test-project", issue, branch, wt_path, str(github_dir)
        )

        reg = await load_registry(registry_path)
        assert workflow_id in reg.worktrees
        assert reg.worktrees[workflow_id].issue == issue

        await cleanup_worktree(workflow_id, registry_path, str(github_dir))

        reg = await load_registry(registry_path)
        assert workflow_id not in reg.worktrees
        assert not wt_path.exists()


class TestWorktreeResilience:
    @pytest.mark.asyncio
    async def test_create_with_stale_directory(self, tmp_path: Path):
        """Stale directory (not tracked by git) should be cleaned up automatically."""
        github_dir = tmp_path
        _repo, branch = await _setup_git_repo(tmp_path)

        # Create a stale directory manually (not a git worktree)
        stale_path = github_dir / "test-project-wt-123"
        stale_path.mkdir()
        (stale_path / "leftover.txt").write_text("stale")

        # create_worktree should remove stale dir and succeed
        wt_path = await create_worktree("test-project", "123", branch, str(github_dir))
        assert wt_path.exists()
        assert not (wt_path / "leftover.txt").exists()

        await remove_worktree(wt_path, github_dir / "test-project")

    @pytest.mark.asyncio
    async def test_create_idempotent_existing_worktree(self, tmp_path: Path):
        """Calling create_worktree on an existing tracked worktree returns the path."""
        github_dir = tmp_path
        _repo, branch = await _setup_git_repo(tmp_path)

        wt_path1 = await create_worktree("test-project", "123", branch, str(github_dir))
        assert wt_path1.exists()

        # Second call with same params should return same path
        wt_path2 = await create_worktree("test-project", "123", branch, str(github_dir))
        assert wt_path2 == wt_path1

        await remove_worktree(wt_path1, github_dir / "test-project")

    @pytest.mark.asyncio
    async def test_register_same_workflow_twice(self, tmp_path: Path):
        """Re-registering same workflow_id should succeed (overwrite)."""
        github_dir = tmp_path
        _repo, branch = await _setup_git_repo(tmp_path)
        wt_path = await create_worktree("test-project", "123", branch, str(github_dir))

        registry_path = tmp_path / "worktrees.json"
        workflow_id = "wf-test-project-123-1000"

        await register_worktree(
            registry_path, workflow_id, "test-project", "123", branch, wt_path, str(github_dir)
        )

        # Register again — should succeed
        await register_worktree(
            registry_path, workflow_id, "test-project", "123", branch, wt_path, str(github_dir)
        )

        reg = await load_registry(registry_path)
        assert workflow_id in reg.worktrees

        await remove_worktree(wt_path, github_dir / "test-project")

    @pytest.mark.asyncio
    async def test_cleanup_missing_directory(self, tmp_path: Path):
        """Cleanup should succeed even if worktree directory was already deleted."""
        github_dir = tmp_path
        _repo, branch = await _setup_git_repo(tmp_path)
        wt_path = await create_worktree("test-project", "123", branch, str(github_dir))

        registry_path = tmp_path / "worktrees.json"
        workflow_id = "wf-test-project-123-1000"

        await register_worktree(
            registry_path, workflow_id, "test-project", "123", branch, wt_path, str(github_dir)
        )

        # Manually delete the directory
        shutil.rmtree(wt_path)
        assert not wt_path.exists()

        # Cleanup should still succeed
        await cleanup_worktree(workflow_id, registry_path, str(github_dir))

        reg = await load_registry(registry_path)
        assert workflow_id not in reg.worktrees

    @pytest.mark.asyncio
    async def test_cleanup_missing_registry_entry(self, tmp_path: Path):
        """Cleanup with unknown workflow_id should succeed (no-op)."""
        registry_path = tmp_path / "worktrees.json"

        # Should not raise
        await cleanup_worktree("wf-nonexistent-1-9999", registry_path, str(tmp_path))

    @pytest.mark.asyncio
    async def test_full_halt_resume_cycle(self, tmp_path: Path):
        """Full cycle: setup -> register -> cleanup -> re-setup -> verify."""
        github_dir = tmp_path
        _repo, branch = await _setup_git_repo(tmp_path)
        registry_path = tmp_path / "worktrees.json"
        workflow_id = "wf-test-project-123-1000"

        # Initial setup
        wt_path = await create_worktree("test-project", "123", branch, str(github_dir))
        await register_worktree(
            registry_path, workflow_id, "test-project", "123", branch, wt_path, str(github_dir)
        )

        # Cleanup (simulates halt)
        await cleanup_worktree(workflow_id, registry_path, str(github_dir))
        assert not wt_path.exists()

        # Re-setup (simulates resume)
        wt_path2 = await create_worktree("test-project", "123", branch, str(github_dir))
        await register_worktree(
            registry_path, workflow_id, "test-project", "123", branch, wt_path2, str(github_dir)
        )

        # Verify
        reg = await load_registry(registry_path)
        assert workflow_id in reg.worktrees
        assert wt_path2.exists()

        await remove_worktree(wt_path2, github_dir / "test-project")


class TestWorktreeRegistryEdgeCases:
    @pytest.mark.asyncio
    async def test_load_registry_missing_file(self, tmp_path: Path):
        """Loading from a non-existent path returns empty registry."""
        reg = await load_registry(tmp_path / "nonexistent.json")
        assert reg.worktrees == {}

    @pytest.mark.asyncio
    async def test_load_registry_corrupted_file(self, tmp_path: Path):
        """Corrupted JSON returns empty registry (graceful recovery)."""
        bad_file = tmp_path / "worktrees.json"
        bad_file.write_text("{invalid json!!!")
        reg = await load_registry(bad_file)
        assert reg.worktrees == {}

    @pytest.mark.asyncio
    async def test_save_and_load_roundtrip(self, tmp_path: Path):
        """Registry can be saved and loaded back."""

        registry_path = tmp_path / "worktrees.json"
        reg = WorktreeRegistry()
        reg.worktrees["wf-test-1-100"] = WorktreeEntry(
            project="test",
            issue="1",
            branch="feature-1",
            path="/tmp/test-wt-1",
            created_at="2026-01-01T00:00:00",
        )
        await save_registry(registry_path, reg)

        loaded = await load_registry(registry_path)
        assert "wf-test-1-100" in loaded.worktrees
        assert loaded.worktrees["wf-test-1-100"].branch == "feature-1"


class TestWorktreeIsolation:
    @pytest.mark.asyncio
    async def test_branch_conflict_detected(self, tmp_path: Path):
        """Registering a branch already in use by another workflow raises."""

        reg = WorktreeRegistry()
        reg.worktrees["wf-existing-1-100"] = WorktreeEntry(
            project="test",
            issue="1",
            branch="shared-branch",
            path="/tmp/test-wt-1",
            created_at="2026-01-01T00:00:00",
        )

        with pytest.raises(RuntimeError, match="Branch shared-branch already in use"):
            await check_isolation(
                reg,
                "wf-new-2-200",
                "shared-branch",
                Path("/tmp/test-wt-2"),
                str(tmp_path),
            )

    @pytest.mark.asyncio
    async def test_path_conflict_detected(self, tmp_path: Path):
        """Registering a path already in use by another workflow raises."""

        reg = WorktreeRegistry()
        reg.worktrees["wf-existing-1-100"] = WorktreeEntry(
            project="test",
            issue="1",
            branch="branch-a",
            path="/tmp/test-wt-1",
            created_at="2026-01-01T00:00:00",
        )

        with pytest.raises(RuntimeError, match=r"Worktree path .* already in use"):
            await check_isolation(
                reg,
                "wf-new-2-200",
                "branch-b",
                Path("/tmp/test-wt-1"),
                str(tmp_path),
            )

    @pytest.mark.asyncio
    async def test_main_copy_rejected(self, tmp_path: Path):
        """Path without -wt- suffix is rejected as a main working copy."""
        reg = WorktreeRegistry()
        with pytest.raises(RuntimeError, match="appears to be a main working copy"):
            await check_isolation(
                reg,
                "wf-test-1-100",
                "feature-1",
                Path("/tmp/test-project"),
                str(tmp_path),
            )

    @pytest.mark.asyncio
    async def test_same_workflow_skipped(self, tmp_path: Path):
        """Re-registration of the same workflow_id is allowed (no conflict)."""

        reg = WorktreeRegistry()
        reg.worktrees["wf-test-1-100"] = WorktreeEntry(
            project="test",
            issue="1",
            branch="feature-1",
            path="/tmp/test-wt-1",
            created_at="2026-01-01T00:00:00",
        )

        # Should not raise — same workflow re-registering
        await check_isolation(
            reg,
            "wf-test-1-100",
            "feature-1",
            Path("/tmp/test-wt-1"),
            str(tmp_path),
        )

    @pytest.mark.asyncio
    async def test_empty_registry_passes(self, tmp_path: Path):
        """Empty registry has no conflicts."""
        reg = WorktreeRegistry()
        await check_isolation(
            reg,
            "wf-test-1-100",
            "feature-1",
            Path("/tmp/test-wt-1"),
            str(tmp_path),
        )

    @pytest.mark.asyncio
    async def test_wrong_branch_worktree_raises(self, tmp_path: Path):
        """Existing worktree on wrong branch raises RuntimeError."""
        github_dir = tmp_path
        repo, branch = await _setup_git_repo(tmp_path)

        # Create worktree on feature-123
        wt_path = await create_worktree("test-project", "123", branch, str(github_dir))

        # Create another branch
        proc = await asyncio.create_subprocess_exec(
            "git",
            "branch",
            "different-branch",
            cwd=str(repo),
            stdout=asyncio.subprocess.DEVNULL,
            stderr=asyncio.subprocess.DEVNULL,
        )
        await proc.wait()

        # Try to create worktree at same path but different branch
        with pytest.raises(RuntimeError, match="exists on branch"):
            await create_worktree("test-project", "123", "different-branch", str(github_dir))

        await remove_worktree(wt_path, repo)
