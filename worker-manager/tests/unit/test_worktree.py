"""Tests for worktree module."""

from pathlib import Path

import pytest

from worker_manager.schemas import WorktreeEntry, WorktreeRegistry
from worker_manager.worktree import (
    check_isolation,
    load_registry,
    save_registry,
)


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
                    issue=144,
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
                    issue=1,
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
        await check_isolation(reg, "feature-branch", wt_path, str(tmp_path))

    @pytest.mark.asyncio
    async def test_duplicate_branch(self, tmp_path: Path):
        reg = WorktreeRegistry(
            worktrees={
                "wf-existing-1-1000": WorktreeEntry(
                    project="test",
                    issue=1,
                    branch="shared-branch",
                    path="/tmp/test-wt-1",
                    created_at="2026-01-01T00:00:00Z",
                )
            }
        )
        wt_path = tmp_path / "project-wt-2"
        with pytest.raises(RuntimeError, match="Branch shared-branch already in use"):
            await check_isolation(reg, "shared-branch", wt_path, str(tmp_path))

    @pytest.mark.asyncio
    async def test_duplicate_path(self, tmp_path: Path):
        reg = WorktreeRegistry(
            worktrees={
                "wf-existing-1-1000": WorktreeEntry(
                    project="test",
                    issue=1,
                    branch="branch-a",
                    path="/tmp/test-wt-1",
                    created_at="2026-01-01T00:00:00Z",
                )
            }
        )
        wt_path = Path("/tmp/test-wt-1")
        with pytest.raises(RuntimeError, match=r"Worktree path .* already in use"):
            await check_isolation(reg, "branch-b", wt_path, str(tmp_path))

    @pytest.mark.asyncio
    async def test_main_working_copy_rejected(self, tmp_path: Path):
        reg = WorktreeRegistry()
        wt_path = tmp_path / "project"  # No -wt- suffix
        with pytest.raises(RuntimeError, match="main working copy"):
            await check_isolation(reg, "some-branch", wt_path, str(tmp_path))
