"""Tests for preflight module."""

from pathlib import Path
from unittest.mock import AsyncMock, patch

import pytest

from worker_manager.preflight import PreflightResult, run_preflight


def _make_proc_mock(stdout=b"", stderr=b"", returncode=0):
    """Create a mock subprocess result."""
    proc = AsyncMock()
    proc.communicate = AsyncMock(return_value=(stdout, stderr))
    proc.returncode = returncode
    return proc


class TestPreflightResult:
    def test_defaults(self):
        r = PreflightResult(passed=True)
        assert r.passed is True
        assert r.repairs == []
        assert r.error is None
        assert r.file_hashes == {}
        assert r.file_backups == {}


class TestRunPreflightBranchCheck:
    @pytest.mark.asyncio
    async def test_correct_branch_no_repair(self, tmp_path: Path):
        worktree = tmp_path / "wt"
        worktree.mkdir()
        spec = worktree / "spec"
        spec.mkdir()
        # Create command file
        cmd_dir = worktree / ".claude" / "commands" / "tanren"
        cmd_dir.mkdir(parents=True)
        (cmd_dir / "do-task.md").write_text("# do-task")

        with patch("worker_manager.preflight.asyncio.create_subprocess_exec") as mock_exec:
            # Branch check returns correct branch
            mock_exec.side_effect = [
                _make_proc_mock(stdout=b"my-branch\n"),  # branch --show-current
                _make_proc_mock(stdout=b""),  # status --porcelain
            ]
            result = await run_preflight(worktree, "my-branch", spec, "do-task")

        assert result.passed is True
        assert len([r for r in result.repairs if "branch" in r.lower()]) == 0

    @pytest.mark.asyncio
    async def test_wrong_branch_recoverable(self, tmp_path: Path):
        worktree = tmp_path / "wt"
        worktree.mkdir()
        spec = worktree / "spec"
        spec.mkdir()
        cmd_dir = worktree / ".claude" / "commands" / "tanren"
        cmd_dir.mkdir(parents=True)
        (cmd_dir / "do-task.md").write_text("# do-task")

        with patch("worker_manager.preflight.asyncio.create_subprocess_exec") as mock_exec:
            mock_exec.side_effect = [
                _make_proc_mock(stdout=b"wrong-branch\n"),  # branch --show-current
                _make_proc_mock(returncode=0),  # checkout succeeds
                _make_proc_mock(stdout=b""),  # status --porcelain
            ]
            result = await run_preflight(worktree, "my-branch", spec, "do-task")

        assert result.passed is True
        assert any("Switched branch" in r for r in result.repairs)

    @pytest.mark.asyncio
    async def test_wrong_branch_unrecoverable(self, tmp_path: Path):
        worktree = tmp_path / "wt"
        worktree.mkdir()
        spec = worktree / "spec"
        spec.mkdir()

        with patch("worker_manager.preflight.asyncio.create_subprocess_exec") as mock_exec:
            mock_exec.side_effect = [
                _make_proc_mock(stdout=b"wrong-branch\n"),  # branch --show-current
                _make_proc_mock(returncode=1, stderr=b"error: cannot checkout"),  # checkout fails
            ]
            result = await run_preflight(worktree, "my-branch", spec, "do-task")

        assert result.passed is False
        assert "Cannot checkout" in result.error


class TestRunPreflightDirtyTree:
    @pytest.mark.asyncio
    async def test_dirty_tree_stashed(self, tmp_path: Path):
        worktree = tmp_path / "wt"
        worktree.mkdir()
        spec = worktree / "spec"
        spec.mkdir()
        cmd_dir = worktree / ".claude" / "commands" / "tanren"
        cmd_dir.mkdir(parents=True)
        (cmd_dir / "do-task.md").write_text("# do-task")

        with patch("worker_manager.preflight.asyncio.create_subprocess_exec") as mock_exec:
            mock_exec.side_effect = [
                _make_proc_mock(stdout=b"my-branch\n"),  # branch
                _make_proc_mock(stdout=b"M file.txt\n"),  # status (dirty)
                _make_proc_mock(returncode=0),  # stash
            ]
            result = await run_preflight(worktree, "my-branch", spec, "do-task")

        assert result.passed is True
        assert any("Stashed" in r for r in result.repairs)


class TestRunPreflightFileSnapshots:
    @pytest.mark.asyncio
    async def test_protected_file_snapshots(self, tmp_path: Path):
        worktree = tmp_path / "wt"
        worktree.mkdir()
        spec = worktree / "spec"
        spec.mkdir()
        cmd_dir = worktree / ".claude" / "commands" / "tanren"
        cmd_dir.mkdir(parents=True)
        (cmd_dir / "do-task.md").write_text("# do-task")

        # Create some snapshot files
        (worktree / "spec.md").write_text("# Spec")
        (worktree / "plan.md").write_text("# Plan")
        # Don't create Makefile — verify it's skipped

        with patch("worker_manager.preflight.asyncio.create_subprocess_exec") as mock_exec:
            mock_exec.side_effect = [
                _make_proc_mock(stdout=b"my-branch\n"),
                _make_proc_mock(stdout=b""),  # clean
            ]
            result = await run_preflight(worktree, "my-branch", spec, "do-task")

        assert "spec.md" in result.file_hashes
        assert "plan.md" in result.file_hashes
        assert "Makefile" not in result.file_hashes
        assert result.file_backups["spec.md"] == "# Spec"
        assert result.file_backups["plan.md"] == "# Plan"


class TestRunPreflightStatusCleared:
    @pytest.mark.asyncio
    async def test_agent_status_cleared(self, tmp_path: Path):
        worktree = tmp_path / "wt"
        worktree.mkdir()
        spec = worktree / "spec"
        spec.mkdir()
        cmd_dir = worktree / ".claude" / "commands" / "tanren"
        cmd_dir.mkdir(parents=True)
        (cmd_dir / "do-task.md").write_text("# do-task")

        # Create status files
        (spec / ".agent-status").write_text("complete")
        (spec / "audit-findings.json").write_text("{}")

        with patch("worker_manager.preflight.asyncio.create_subprocess_exec") as mock_exec:
            mock_exec.side_effect = [
                _make_proc_mock(stdout=b"my-branch\n"),
                _make_proc_mock(stdout=b""),
            ]
            result = await run_preflight(worktree, "my-branch", spec, "do-task")

        assert result.passed is True
        assert not (spec / ".agent-status").exists()
        assert not (spec / "audit-findings.json").exists()
        assert any("Cleared .agent-status" in r for r in result.repairs)


class TestRunPreflightCommandFile:
    @pytest.mark.asyncio
    async def test_command_file_missing(self, tmp_path: Path):
        worktree = tmp_path / "wt"
        worktree.mkdir()
        spec = worktree / "spec"
        spec.mkdir()
        # Don't create command file

        with patch("worker_manager.preflight.asyncio.create_subprocess_exec") as mock_exec:
            mock_exec.side_effect = [
                _make_proc_mock(stdout=b"my-branch\n"),
                _make_proc_mock(stdout=b""),
            ]
            result = await run_preflight(worktree, "my-branch", spec, "do-task")

        assert result.passed is False
        assert "Command file missing" in result.error


class TestRunPreflightInvestigatePhase:
    @pytest.mark.asyncio
    async def test_investigate_checks_command_file(self, tmp_path: Path):
        """investigate phase requires command file like other agent phases."""
        worktree = tmp_path / "wt"
        worktree.mkdir()
        spec = worktree / "spec"
        spec.mkdir()
        # No command file for investigate

        with patch("worker_manager.preflight.asyncio.create_subprocess_exec") as mock_exec:
            mock_exec.side_effect = [
                _make_proc_mock(stdout=b"my-branch\n"),
                _make_proc_mock(stdout=b""),
            ]
            result = await run_preflight(worktree, "my-branch", spec, "investigate")

        assert result.passed is False
        assert "Command file missing" in result.error

    @pytest.mark.asyncio
    async def test_investigate_passes_with_command_file(self, tmp_path: Path):
        worktree = tmp_path / "wt"
        worktree.mkdir()
        spec = worktree / "spec"
        spec.mkdir()
        cmd_dir = worktree / ".claude" / "commands" / "tanren"
        cmd_dir.mkdir(parents=True)
        (cmd_dir / "investigate.md").write_text("# investigate")

        with patch("worker_manager.preflight.asyncio.create_subprocess_exec") as mock_exec:
            mock_exec.side_effect = [
                _make_proc_mock(stdout=b"my-branch\n"),
                _make_proc_mock(stdout=b""),
            ]
            result = await run_preflight(worktree, "my-branch", spec, "investigate")

        assert result.passed is True


class TestRunPreflightCleanPass:
    @pytest.mark.asyncio
    async def test_clean_worktree_passes(self, tmp_path: Path):
        worktree = tmp_path / "wt"
        worktree.mkdir()
        spec = worktree / "spec"
        spec.mkdir()
        cmd_dir = worktree / ".claude" / "commands" / "tanren"
        cmd_dir.mkdir(parents=True)
        (cmd_dir / "do-task.md").write_text("# do-task")

        with patch("worker_manager.preflight.asyncio.create_subprocess_exec") as mock_exec:
            mock_exec.side_effect = [
                _make_proc_mock(stdout=b"my-branch\n"),
                _make_proc_mock(stdout=b""),
            ]
            result = await run_preflight(worktree, "my-branch", spec, "do-task")

        assert result.passed is True
        assert result.repairs == []
        assert result.error is None
