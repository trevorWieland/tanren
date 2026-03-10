"""Integration tests for postflight — uses real git repos."""

import hashlib
import subprocess
from pathlib import Path

import pytest

from worker_manager.postflight import run_postflight


def _hash(content: str) -> str:
    return hashlib.md5(content.encode()).hexdigest()


@pytest.fixture
def git_repo(tmp_path: Path) -> Path:
    """Create a real git repo with initial commit."""
    repo = tmp_path / "repo"
    repo.mkdir()

    for cmd in [
        ["git", "-C", str(repo), "init"],
        ["git", "-C", str(repo), "checkout", "-b", "test-branch"],
        ["git", "-C", str(repo), "config", "user.email", "test@test.com"],
        ["git", "-C", str(repo), "config", "user.name", "Test"],
    ]:
        subprocess.run(cmd, capture_output=True, check=True)

    # Create initial files
    (repo / "spec.md").write_text("# Original Spec")
    (repo / "plan.md").write_text("# Original Plan")
    (repo / "code.py").write_text("print('hello')")

    for cmd in [
        ["git", "-C", str(repo), "add", "-A"],
        ["git", "-C", str(repo), "commit", "-m", "initial"],
    ]:
        subprocess.run(cmd, capture_output=True, check=True)

    return repo


class TestPostflightIntegration:
    @pytest.mark.asyncio
    async def test_spec_reverted_unstaged(self, git_repo: Path):
        """Agent modified spec.md but didn't commit — postflight reverts on disk."""
        original = "# Original Spec"
        original_hash = _hash(original)

        # Simulate agent modifying spec.md (unstaged)
        (git_repo / "spec.md").write_text("# Agent Modified Spec")

        result = await run_postflight(
            git_repo,
            "test-branch",
            "do-task",
            {"spec.md": original_hash},
            {"spec.md": original},
        )

        assert result.integrity_repairs["spec_reverted"] is True
        # Verify file content is reverted
        assert (git_repo / "spec.md").read_text() == original

    @pytest.mark.asyncio
    async def test_spec_reverted_after_agent_committed(self, git_repo: Path):
        """Agent modified AND committed spec.md — postflight reverts with new commit."""
        original = "# Original Spec"
        original_hash = _hash(original)

        # Simulate agent modifying and committing spec.md
        (git_repo / "spec.md").write_text("# Agent Modified Spec")
        for cmd in [
            ["git", "-C", str(git_repo), "add", "spec.md"],
            ["git", "-C", str(git_repo), "commit", "-m", "agent: modify spec"],
        ]:
            subprocess.run(cmd, capture_output=True, check=True)

        result = await run_postflight(
            git_repo,
            "test-branch",
            "do-task",
            {"spec.md": original_hash},
            {"spec.md": original},
        )

        assert result.integrity_repairs["spec_reverted"] is True
        assert (git_repo / "spec.md").read_text() == original

        # Verify a revert commit was created (separate from agent's commit)
        proc = subprocess.run(
            ["git", "-C", str(git_repo), "log", "--oneline", "-1"],
            capture_output=True,
            text=True,
        )
        assert "postflight: revert" in proc.stdout

    @pytest.mark.asyncio
    async def test_uncommitted_work_saved(self, git_repo: Path):
        """Dirty files after agent should be committed as WIP."""
        # Simulate agent leaving uncommitted work
        (git_repo / "new_file.py").write_text("# agent work")

        result = await run_postflight(git_repo, "test-branch", "do-task", {}, {})

        assert result.integrity_repairs["wip_committed"] is True

        # Verify WIP commit exists
        proc = subprocess.run(
            ["git", "-C", str(git_repo), "log", "--oneline", "-1"],
            capture_output=True,
            text=True,
        )
        assert "WIP" in proc.stdout

    @pytest.mark.asyncio
    async def test_no_changes_clean(self, git_repo: Path):
        """No modifications — no repairs needed (push will fail without remote, that's OK)."""
        result = await run_postflight(git_repo, "test-branch", "do-task", {}, {})

        assert result.integrity_repairs["spec_reverted"] is False
        assert result.integrity_repairs["wip_committed"] is False
        # Push will fail without a remote, but that's expected in test
        assert result.pushed is False

    @pytest.mark.asyncio
    async def test_skip_push_integrity_still_works(self, git_repo: Path):
        """skip_push=True skips push but integrity checks still run."""
        original = "# Original Spec"
        original_hash = _hash(original)

        # Simulate agent modifying spec.md
        (git_repo / "spec.md").write_text("# Agent Modified")
        # Also leave uncommitted work
        (git_repo / "extra.py").write_text("# extra")

        result = await run_postflight(
            git_repo,
            "test-branch",
            "do-task",
            {"spec.md": original_hash},
            {"spec.md": original},
            skip_push=True,
        )

        # Integrity ran: spec reverted and WIP committed
        assert result.integrity_repairs["spec_reverted"] is True
        assert result.integrity_repairs["wip_committed"] is True
        assert (git_repo / "spec.md").read_text() == original
        # Push was skipped
        assert result.pushed is False
        assert result.push_error is None
