"""Tests for worktree module."""

import asyncio
from pathlib import Path

import pytest

from tanren_core.schemas import WorktreeEntry, WorktreeRegistry
from tanren_core.worktree import (
    check_isolation,
    get_default_branch,
    load_registry,
    save_registry,
)


async def _init_repo(repo: Path, default_branch: str = "main") -> None:
    """Initialize a git repo with one commit."""
    repo.mkdir(parents=True, exist_ok=True)  # noqa: ASYNC240 — trivial sync fs op after async work
    for cmd in [
        ["git", "init"],
        ["git", "config", "user.email", "test@test.com"],
        ["git", "config", "user.name", "Test"],
        ["git", "checkout", "-b", default_branch],
    ]:
        proc = await asyncio.create_subprocess_exec(
            *cmd,
            cwd=str(repo),
            stdout=asyncio.subprocess.DEVNULL,
            stderr=asyncio.subprocess.DEVNULL,
        )
        await proc.wait()
    (repo / "README.md").write_text("# Test")
    for cmd in [["git", "add", "."], ["git", "commit", "-m", "init"]]:
        proc = await asyncio.create_subprocess_exec(
            *cmd,
            cwd=str(repo),
            stdout=asyncio.subprocess.DEVNULL,
            stderr=asyncio.subprocess.DEVNULL,
        )
        await proc.wait()


class TestGetDefaultBranch:
    @pytest.mark.asyncio
    async def test_fallback_to_main(self, tmp_path: Path):
        repo = tmp_path / "repo"
        await _init_repo(repo, "main")
        result = await get_default_branch(repo)
        assert result == "main"

    @pytest.mark.asyncio
    async def test_fallback_to_master(self, tmp_path: Path):
        repo = tmp_path / "repo"
        await _init_repo(repo, "master")
        result = await get_default_branch(repo)
        assert result == "master"

    @pytest.mark.asyncio
    async def test_no_default_raises(self, tmp_path: Path):
        repo = tmp_path / "repo"
        await _init_repo(repo, "develop")
        with pytest.raises(RuntimeError, match="Cannot determine default branch"):
            await get_default_branch(repo)


class TestLoadRegistry:
    @pytest.mark.asyncio
    async def test_missing_file(self, tmp_path: Path):
        reg = await load_registry(tmp_path / "nonexistent.json")
        assert reg.worktrees == {}

    @pytest.mark.asyncio
    async def test_empty_registry(self, tmp_path: Path):
        path = tmp_path / "worktrees.json"
        path.write_text('{"worktrees": {}}')
        reg = await load_registry(path)
        assert reg.worktrees == {}

    @pytest.mark.asyncio
    async def test_with_entries(self, tmp_path: Path):
        path = tmp_path / "worktrees.json"
        reg = WorktreeRegistry(
            worktrees={
                "wf-rentl-144-1741359600": WorktreeEntry(
                    project="rentl",
                    issue="144",
                    branch="s0146-slug",
                    path="/home/trevor/github/rentl-wt-144",
                    created_at="2026-03-07T15:01:00Z",
                )
            }
        )
        path.write_text(reg.model_dump_json())
        loaded = await load_registry(path)
        assert "wf-rentl-144-1741359600" in loaded.worktrees

    @pytest.mark.asyncio
    async def test_invalid_json_returns_empty(self, tmp_path: Path):
        path = tmp_path / "worktrees.json"
        path.write_text("not json")
        reg = await load_registry(path)
        assert reg.worktrees == {}


class TestSaveRegistry:
    @pytest.mark.asyncio
    async def test_creates_parent_dirs(self, tmp_path: Path):
        path = tmp_path / "subdir" / "worktrees.json"
        reg = WorktreeRegistry()
        await save_registry(path, reg)
        assert path.exists()

    @pytest.mark.asyncio
    async def test_roundtrip(self, tmp_path: Path):
        path = tmp_path / "worktrees.json"
        reg = WorktreeRegistry(
            worktrees={
                "wf-test-1-1000": WorktreeEntry(
                    project="test",
                    issue="1",
                    branch="feat-1",
                    path="/tmp/test-wt-1",
                    created_at="2026-01-01T00:00:00Z",
                )
            }
        )
        await save_registry(path, reg)
        loaded = await load_registry(path)
        assert loaded.worktrees["wf-test-1-1000"].branch == "feat-1"


class TestCheckIsolation:
    @pytest.mark.asyncio
    async def test_no_conflict(self, tmp_path: Path):
        reg = WorktreeRegistry()
        wt_path = tmp_path / "project-wt-1"
        await check_isolation(reg, "wf-new-1-1000", "feature-branch", wt_path, str(tmp_path))

    @pytest.mark.asyncio
    async def test_duplicate_branch(self, tmp_path: Path):
        reg = WorktreeRegistry(
            worktrees={
                "wf-existing-1-1000": WorktreeEntry(
                    project="test",
                    issue="1",
                    branch="shared-branch",
                    path="/tmp/test-wt-1",
                    created_at="2026-01-01T00:00:00Z",
                )
            }
        )
        wt_path = tmp_path / "project-wt-2"
        with pytest.raises(RuntimeError, match="Branch shared-branch already in use"):
            await check_isolation(reg, "wf-other-2-2000", "shared-branch", wt_path, str(tmp_path))

    @pytest.mark.asyncio
    async def test_duplicate_path(self, tmp_path: Path):
        reg = WorktreeRegistry(
            worktrees={
                "wf-existing-1-1000": WorktreeEntry(
                    project="test",
                    issue="1",
                    branch="branch-a",
                    path="/tmp/test-wt-1",
                    created_at="2026-01-01T00:00:00Z",
                )
            }
        )
        wt_path = Path("/tmp/test-wt-1")
        with pytest.raises(RuntimeError, match=r"Worktree path .* already in use"):
            await check_isolation(reg, "wf-other-2-2000", "branch-b", wt_path, str(tmp_path))

    @pytest.mark.asyncio
    async def test_main_working_copy_rejected(self, tmp_path: Path):
        reg = WorktreeRegistry()
        wt_path = tmp_path / "project"  # No -wt- suffix
        with pytest.raises(RuntimeError, match="main working copy"):
            await check_isolation(reg, "wf-new-1-1000", "some-branch", wt_path, str(tmp_path))

    @pytest.mark.asyncio
    async def test_same_workflow_allowed(self, tmp_path: Path):
        """Same workflow re-registering should not raise."""
        reg = WorktreeRegistry(
            worktrees={
                "wf-test-1-1000": WorktreeEntry(
                    project="test",
                    issue="1",
                    branch="feat-1",
                    path="/tmp/test-wt-1",
                    created_at="2026-01-01T00:00:00Z",
                )
            }
        )
        wt_path = Path("/tmp/test-wt-1")
        # Same workflow_id, same branch — should NOT raise
        await check_isolation(reg, "wf-test-1-1000", "feat-1", wt_path, str(tmp_path))

    @pytest.mark.asyncio
    async def test_different_workflow_same_branch_rejected(self, tmp_path: Path):
        """Different workflow using same branch should raise."""
        reg = WorktreeRegistry(
            worktrees={
                "wf-test-1-1000": WorktreeEntry(
                    project="test",
                    issue="1",
                    branch="feat-1",
                    path="/tmp/test-wt-1",
                    created_at="2026-01-01T00:00:00Z",
                )
            }
        )
        wt_path = tmp_path / "project-wt-2"
        with pytest.raises(RuntimeError, match="Branch feat-1 already in use"):
            await check_isolation(reg, "wf-test-2-2000", "feat-1", wt_path, str(tmp_path))
