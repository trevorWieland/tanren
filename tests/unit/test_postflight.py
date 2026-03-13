"""Tests for postflight module."""

import hashlib
from pathlib import Path
from unittest.mock import AsyncMock, patch

import pytest

from tanren_core.postflight import PostflightResult, run_postflight


def _make_proc_mock(stdout=b"", stderr=b"", returncode=0):
    """Create a mock subprocess result."""
    proc = AsyncMock()
    proc.communicate = AsyncMock(return_value=(stdout, stderr))
    proc.returncode = returncode
    return proc


def _hash(content: str) -> str:
    return hashlib.md5(content.encode()).hexdigest()


class TestPostflightResult:
    def test_defaults(self):
        r = PostflightResult()
        assert r.pushed is False
        assert r.push_error is None
        assert r.integrity_repairs.branch_switched is False
        assert r.integrity_repairs.spec_reverted is False
        assert r.integrity_repairs.plan_reverted is False
        assert r.integrity_repairs.wip_committed is False


class TestBranchRecovery:
    @pytest.mark.asyncio
    async def test_branch_switched_recovery(self, tmp_path: Path):
        worktree = tmp_path / "wt"
        worktree.mkdir()

        with patch("tanren_core.postflight.asyncio.create_subprocess_exec") as mock_exec:
            mock_exec.side_effect = [
                _make_proc_mock(stdout=b"wrong-branch\n"),  # branch check
                _make_proc_mock(returncode=0),  # checkout
                _make_proc_mock(stdout=b""),  # status --porcelain (clean)
                _make_proc_mock(returncode=0),  # push
            ]
            result = await run_postflight(worktree, "my-branch", "do-task", {}, {})

        assert result.integrity_repairs.branch_switched is True


class TestSpecRevert:
    @pytest.mark.asyncio
    async def test_spec_reverted(self, tmp_path: Path):
        worktree = tmp_path / "wt"
        worktree.mkdir()
        original_spec = "# Original spec"
        modified_spec = "# Modified spec"
        (worktree / "spec.md").write_text(modified_spec)

        hashes = {"spec.md": _hash(original_spec)}
        backups = {"spec.md": original_spec}

        with patch("tanren_core.postflight.asyncio.create_subprocess_exec") as mock_exec:
            mock_exec.side_effect = [
                _make_proc_mock(stdout=b"my-branch\n"),  # branch check
                _make_proc_mock(returncode=0),  # git add
                _make_proc_mock(returncode=0),  # git commit (revert)
                _make_proc_mock(stdout=b""),  # status (clean)
                _make_proc_mock(returncode=0),  # push
            ]
            result = await run_postflight(worktree, "my-branch", "do-task", hashes, backups)

        assert result.integrity_repairs.spec_reverted is True
        # Verify file was reverted on disk
        assert (worktree / "spec.md").read_text() == original_spec

    @pytest.mark.asyncio
    async def test_spec_always_reverted_even_in_audit(self, tmp_path: Path):
        """spec.md is always protected, even during audit phases."""
        worktree = tmp_path / "wt"
        worktree.mkdir()
        original = "# Original"
        (worktree / "spec.md").write_text("# Modified")

        with patch("tanren_core.postflight.asyncio.create_subprocess_exec") as mock_exec:
            mock_exec.side_effect = [
                _make_proc_mock(stdout=b"my-branch\n"),
                _make_proc_mock(returncode=0),  # git add
                _make_proc_mock(returncode=0),  # git commit
                _make_proc_mock(stdout=b""),  # status
                _make_proc_mock(returncode=0),  # push
            ]
            result = await run_postflight(
                worktree,
                "my-branch",
                "audit-task",
                {"spec.md": _hash(original)},
                {"spec.md": original},
            )

        assert result.integrity_repairs.spec_reverted is True


class TestPlanRevert:
    @pytest.mark.asyncio
    async def test_plan_reverted_do_task(self, tmp_path: Path):
        """plan.md is protected during do-task (implementation phase)."""
        worktree = tmp_path / "wt"
        worktree.mkdir()
        original = "# Original plan"
        (worktree / "plan.md").write_text("# Modified plan")

        with patch("tanren_core.postflight.asyncio.create_subprocess_exec") as mock_exec:
            mock_exec.side_effect = [
                _make_proc_mock(stdout=b"my-branch\n"),
                _make_proc_mock(returncode=0),  # git add
                _make_proc_mock(returncode=0),  # git commit
                _make_proc_mock(stdout=b""),  # status
                _make_proc_mock(returncode=0),  # push
            ]
            result = await run_postflight(
                worktree,
                "my-branch",
                "do-task",
                {"plan.md": _hash(original)},
                {"plan.md": original},
            )

        assert result.integrity_repairs.plan_reverted is True
        assert (worktree / "plan.md").read_text() == original

    @pytest.mark.asyncio
    async def test_plan_not_reverted_audit_task(self, tmp_path: Path):
        """plan.md is NOT protected during audit-task (audit phase)."""
        worktree = tmp_path / "wt"
        worktree.mkdir()
        original = "# Original plan"
        modified = "# Plan with fix items"
        (worktree / "plan.md").write_text(modified)

        with patch("tanren_core.postflight.asyncio.create_subprocess_exec") as mock_exec:
            mock_exec.side_effect = [
                _make_proc_mock(stdout=b"my-branch\n"),
                _make_proc_mock(stdout=b""),  # status
                _make_proc_mock(returncode=0),  # push
            ]
            result = await run_postflight(
                worktree,
                "my-branch",
                "audit-task",
                {"plan.md": _hash(original)},
                {"plan.md": original},
            )

        assert result.integrity_repairs.plan_reverted is False
        # plan.md should still have the modified content
        assert (worktree / "plan.md").read_text() == modified

    @pytest.mark.asyncio
    async def test_plan_not_reverted_audit_spec(self, tmp_path: Path):
        """plan.md is NOT protected during audit-spec."""
        worktree = tmp_path / "wt"
        worktree.mkdir()
        original = "# Original plan"
        modified = "# Plan with fix items"
        (worktree / "plan.md").write_text(modified)

        with patch("tanren_core.postflight.asyncio.create_subprocess_exec") as mock_exec:
            mock_exec.side_effect = [
                _make_proc_mock(stdout=b"my-branch\n"),
                _make_proc_mock(stdout=b""),  # status
                _make_proc_mock(returncode=0),  # push
            ]
            result = await run_postflight(
                worktree,
                "my-branch",
                "audit-spec",
                {"plan.md": _hash(original)},
                {"plan.md": original},
            )

        assert result.integrity_repairs.plan_reverted is False


class TestWarnOnly:
    @pytest.mark.asyncio
    async def test_makefile_warning_only(self, tmp_path: Path):
        """Makefile is warn-only — never reverted."""
        worktree = tmp_path / "wt"
        worktree.mkdir()
        original = "all: build"
        modified = "all: build test"
        (worktree / "Makefile").write_text(modified)

        with patch("tanren_core.postflight.asyncio.create_subprocess_exec") as mock_exec:
            mock_exec.side_effect = [
                _make_proc_mock(stdout=b"my-branch\n"),
                _make_proc_mock(stdout=b""),  # status
                _make_proc_mock(returncode=0),  # push
            ]
            result = await run_postflight(
                worktree,
                "my-branch",
                "do-task",
                {"Makefile": _hash(original)},
                {"Makefile": original},
            )

        assert result.integrity_repairs.makefile_modified is True
        # Makefile should NOT be reverted
        assert (worktree / "Makefile").read_text() == modified


class TestRevertCommit:
    @pytest.mark.asyncio
    async def test_revert_creates_separate_commit(self, tmp_path: Path):
        """Verify postflight creates a new commit (not amend) for reverts."""
        worktree = tmp_path / "wt"
        worktree.mkdir()
        original = "# Original"
        (worktree / "spec.md").write_text("# Modified")

        with patch("tanren_core.postflight.asyncio.create_subprocess_exec") as mock_exec:
            mock_exec.side_effect = [
                _make_proc_mock(stdout=b"my-branch\n"),
                _make_proc_mock(returncode=0),  # git add
                _make_proc_mock(returncode=0),  # git commit
                _make_proc_mock(stdout=b""),  # status
                _make_proc_mock(returncode=0),  # push
            ]
            await run_postflight(
                worktree,
                "my-branch",
                "do-task",
                {"spec.md": _hash(original)},
                {"spec.md": original},
            )

        # Verify the commit command was called with -m (not --amend)
        commit_calls = [
            c for c in mock_exec.call_args_list if "commit" in c.args and "-m" in c.args
        ]
        assert len(commit_calls) == 1
        assert "--amend" not in commit_calls[0].args


class TestUncommittedWork:
    @pytest.mark.asyncio
    async def test_uncommitted_work_committed(self, tmp_path: Path):
        worktree = tmp_path / "wt"
        worktree.mkdir()

        with patch("tanren_core.postflight.asyncio.create_subprocess_exec") as mock_exec:
            mock_exec.side_effect = [
                _make_proc_mock(stdout=b"my-branch\n"),
                _make_proc_mock(stdout=b"M dirty.txt\n"),  # status (dirty)
                _make_proc_mock(returncode=0),  # git add -A
                _make_proc_mock(returncode=0),  # git commit WIP
                _make_proc_mock(returncode=0),  # push
            ]
            result = await run_postflight(worktree, "my-branch", "do-task", {}, {})

        assert result.integrity_repairs.wip_committed is True


class TestPush:
    @pytest.mark.asyncio
    async def test_push_success(self, tmp_path: Path):
        worktree = tmp_path / "wt"
        worktree.mkdir()

        with patch("tanren_core.postflight.asyncio.create_subprocess_exec") as mock_exec:
            mock_exec.side_effect = [
                _make_proc_mock(stdout=b"my-branch\n"),
                _make_proc_mock(stdout=b""),  # status (clean)
                _make_proc_mock(returncode=0),  # push
            ]
            result = await run_postflight(worktree, "my-branch", "do-task", {}, {})

        assert result.pushed is True
        assert result.push_error is None

    @pytest.mark.asyncio
    async def test_push_failure(self, tmp_path: Path):
        worktree = tmp_path / "wt"
        worktree.mkdir()

        with patch("tanren_core.postflight.asyncio.create_subprocess_exec") as mock_exec:
            mock_exec.side_effect = [
                _make_proc_mock(stdout=b"my-branch\n"),
                _make_proc_mock(stdout=b""),  # status (clean)
                _make_proc_mock(returncode=1, stderr=b"remote rejected"),  # push fails
            ]
            result = await run_postflight(worktree, "my-branch", "do-task", {}, {})

        assert result.pushed is False
        assert "remote rejected" in result.push_error


class TestSkipPush:
    @pytest.mark.asyncio
    async def test_skip_push_true(self, tmp_path: Path):
        """When skip_push=True, push step is skipped entirely."""
        worktree = tmp_path / "wt"
        worktree.mkdir()

        with patch("tanren_core.postflight.asyncio.create_subprocess_exec") as mock_exec:
            mock_exec.side_effect = [
                _make_proc_mock(stdout=b"my-branch\n"),  # branch check
                _make_proc_mock(stdout=b""),  # status (clean)
                # No push mock — should not be called
            ]
            result = await run_postflight(worktree, "my-branch", "do-task", {}, {}, skip_push=True)

        assert result.pushed is False  # Default, push was skipped
        assert result.push_error is None

    @pytest.mark.asyncio
    async def test_skip_push_false_default(self, tmp_path: Path):
        """Default skip_push=False still pushes."""
        worktree = tmp_path / "wt"
        worktree.mkdir()

        with patch("tanren_core.postflight.asyncio.create_subprocess_exec") as mock_exec:
            mock_exec.side_effect = [
                _make_proc_mock(stdout=b"my-branch\n"),
                _make_proc_mock(stdout=b""),
                _make_proc_mock(returncode=0),  # push
            ]
            result = await run_postflight(worktree, "my-branch", "do-task", {}, {})

        assert result.pushed is True

    @pytest.mark.asyncio
    async def test_skip_push_integrity_still_runs(self, tmp_path: Path):
        """With skip_push=True, integrity checks (branch, files, WIP) still run."""
        worktree = tmp_path / "wt"
        worktree.mkdir()
        original = "# Original"
        (worktree / "spec.md").write_text("# Modified")

        with patch("tanren_core.postflight.asyncio.create_subprocess_exec") as mock_exec:
            mock_exec.side_effect = [
                _make_proc_mock(stdout=b"my-branch\n"),  # branch check
                _make_proc_mock(returncode=0),  # git add (revert)
                _make_proc_mock(returncode=0),  # git commit (revert)
                _make_proc_mock(stdout=b""),  # status (clean after revert)
                # No push — skip_push=True
            ]
            result = await run_postflight(
                worktree,
                "my-branch",
                "do-task",
                {"spec.md": _hash(original)},
                {"spec.md": original},
                skip_push=True,
            )

        assert result.integrity_repairs.spec_reverted is True
        assert (worktree / "spec.md").read_text() == original
        assert result.pushed is False


class TestNoRepairsNeeded:
    @pytest.mark.asyncio
    async def test_clean_state(self, tmp_path: Path):
        worktree = tmp_path / "wt"
        worktree.mkdir()

        with patch("tanren_core.postflight.asyncio.create_subprocess_exec") as mock_exec:
            mock_exec.side_effect = [
                _make_proc_mock(stdout=b"my-branch\n"),
                _make_proc_mock(stdout=b""),
                _make_proc_mock(returncode=0),
            ]
            result = await run_postflight(worktree, "my-branch", "do-task", {}, {})

        assert result.integrity_repairs.branch_switched is False
        assert result.integrity_repairs.spec_reverted is False
        assert result.integrity_repairs.plan_reverted is False
        assert result.integrity_repairs.wip_committed is False
        assert result.pushed is True
