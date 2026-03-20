"""Integration tests for postflight edge cases using real git repos."""

import asyncio
import hashlib
import subprocess
from typing import TYPE_CHECKING

import pytest

from tanren_core.postflight import run_postflight

if TYPE_CHECKING:
    from pathlib import Path


@pytest.fixture
def postflight_repo(tmp_path: Path) -> Path:
    """Create a git repo suitable for postflight testing."""
    repo = tmp_path / "repo"
    repo.mkdir()

    for cmd in [
        ["git", "-C", str(repo), "init"],
        ["git", "-C", str(repo), "checkout", "-b", "test-branch"],
        ["git", "-C", str(repo), "config", "user.email", "test@test.com"],
        ["git", "-C", str(repo), "config", "user.name", "Test"],
    ]:
        subprocess.run(cmd, capture_output=True, check=True)

    (repo / "spec.md").write_text("# Spec\nOriginal content")
    (repo / "plan.md").write_text("# Plan\nOriginal plan")
    (repo / "Makefile").write_text("all:\n\techo ok")
    (repo / "main.py").write_text("print('hello')")

    for cmd in [
        ["git", "-C", str(repo), "add", "-A"],
        ["git", "-C", str(repo), "commit", "-m", "initial"],
    ]:
        subprocess.run(cmd, capture_output=True, check=True)

    return repo


def _md5(content: str) -> str:
    return hashlib.md5(content.encode()).hexdigest()


class TestPostflightNoChanges:
    @pytest.mark.asyncio
    async def test_no_file_changes_no_repairs(self, postflight_repo: Path):
        """Postflight with no file changes results in no repairs."""
        spec_content = "# Spec\nOriginal content"
        plan_content = "# Plan\nOriginal plan"
        hashes = {"spec.md": _md5(spec_content), "plan.md": _md5(plan_content)}
        backups = {"spec.md": spec_content, "plan.md": plan_content}

        result = await run_postflight(
            postflight_repo,
            "test-branch",
            "do-task",
            hashes,
            backups,
            skip_push=True,
        )

        assert not result.integrity_repairs.spec_reverted
        assert not result.integrity_repairs.plan_reverted
        assert not result.integrity_repairs.branch_switched
        assert not result.integrity_repairs.wip_committed


class TestPostflightSpecRevert:
    @pytest.mark.asyncio
    async def test_spec_md_reverted(self, postflight_repo: Path):
        """Unauthorized spec.md modification is reverted."""
        original = "# Spec\nOriginal content"
        hashes = {"spec.md": _md5(original)}
        backups = {"spec.md": original}

        # Agent modifies spec.md
        (postflight_repo / "spec.md").write_text("# Spec\nModified by agent")

        result = await run_postflight(
            postflight_repo,
            "test-branch",
            "do-task",
            hashes,
            backups,
            skip_push=True,
        )

        assert result.integrity_repairs.spec_reverted
        # Verify file was actually reverted
        assert (postflight_repo / "spec.md").read_text() == original

    @pytest.mark.asyncio
    async def test_plan_md_reverted_in_impl_phase(self, postflight_repo: Path):
        """plan.md is reverted during do-task (implementation phase)."""
        original = "# Plan\nOriginal plan"
        hashes = {"plan.md": _md5(original)}
        backups = {"plan.md": original}

        (postflight_repo / "plan.md").write_text("# Plan\nModified by agent")

        result = await run_postflight(
            postflight_repo,
            "test-branch",
            "do-task",
            hashes,
            backups,
            skip_push=True,
        )

        assert result.integrity_repairs.plan_reverted
        assert (postflight_repo / "plan.md").read_text() == original

    @pytest.mark.asyncio
    async def test_plan_md_not_reverted_in_audit_phase(self, postflight_repo: Path):
        """plan.md is NOT reverted during audit-task (audits may append fix items)."""
        original = "# Plan\nOriginal plan"
        hashes = {"plan.md": _md5(original)}
        backups = {"plan.md": original}

        modified = "# Plan\nModified by audit"
        (postflight_repo / "plan.md").write_text(modified)

        result = await run_postflight(
            postflight_repo,
            "test-branch",
            "audit-task",
            hashes,
            backups,
            skip_push=True,
        )

        assert not result.integrity_repairs.plan_reverted
        # File should remain modified
        assert (postflight_repo / "plan.md").read_text() == modified


class TestPostflightSkipPush:
    @pytest.mark.asyncio
    async def test_skip_push_flag(self, postflight_repo: Path):
        """skip_push=True prevents git push."""
        result = await run_postflight(
            postflight_repo,
            "test-branch",
            "do-task",
            {},
            {},
            skip_push=True,
        )

        assert not result.pushed
        assert result.push_error is None


class TestPostflightWarnOnly:
    @pytest.mark.asyncio
    async def test_makefile_modification_warns(self, postflight_repo: Path):
        """Makefile modification is detected and flagged (not reverted)."""
        original = "all:\n\techo ok"
        hashes = {"Makefile": _md5(original)}

        (postflight_repo / "Makefile").write_text("all:\n\techo modified")

        result = await run_postflight(
            postflight_repo,
            "test-branch",
            "do-task",
            hashes,
            {},
            skip_push=True,
        )

        assert result.integrity_repairs.makefile_modified
        # Should NOT be reverted (warn-only)
        assert (postflight_repo / "Makefile").read_text() == "all:\n\techo modified"


class TestPostflightUncommittedWork:
    @pytest.mark.asyncio
    async def test_uncommitted_work_committed_as_wip(self, postflight_repo: Path):
        """Uncommitted changes are committed as WIP."""
        (postflight_repo / "new_file.py").write_text("# new file")

        result = await run_postflight(
            postflight_repo,
            "test-branch",
            "do-task",
            {},
            {},
            skip_push=True,
        )

        assert result.integrity_repairs.wip_committed

        # Verify the WIP commit was made
        proc = await asyncio.create_subprocess_exec(
            "git",
            "-C",
            str(postflight_repo),
            "log",
            "--oneline",
            "-1",
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        stdout, _ = await proc.communicate()
        assert "WIP" in stdout.decode()

    @pytest.mark.asyncio
    async def test_clean_tree_no_wip(self, postflight_repo: Path):
        """Clean working tree does not create WIP commit."""
        result = await run_postflight(
            postflight_repo,
            "test-branch",
            "do-task",
            {},
            {},
            skip_push=True,
        )

        assert not result.integrity_repairs.wip_committed


class TestPostflightMissingFile:
    @pytest.mark.asyncio
    async def test_missing_protected_file_skipped(self, postflight_repo: Path):
        """If a protected file doesn't exist, it's silently skipped."""
        hashes = {"nonexistent.md": "abc123"}
        backups = {"nonexistent.md": "content"}

        # Should not raise
        result = await run_postflight(
            postflight_repo,
            "test-branch",
            "do-task",
            hashes,
            backups,
            skip_push=True,
        )

        assert not result.integrity_repairs.spec_reverted
