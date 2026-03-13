"""Integration tests for preflight — uses real git repos."""

import asyncio
import subprocess
from pathlib import Path

import pytest

from tanren_core.preflight import run_preflight


@pytest.fixture
def git_repo(tmp_path: Path) -> Path:
    """Create a real git repo with a branch and initial commit."""
    repo = tmp_path / "repo"
    repo.mkdir()

    for cmd in [
        ["git", "-C", str(repo), "init"],
        ["git", "-C", str(repo), "checkout", "-b", "test-branch"],
        ["git", "-C", str(repo), "config", "user.email", "test@test.com"],
        ["git", "-C", str(repo), "config", "user.name", "Test"],
    ]:
        subprocess.run(cmd, capture_output=True, check=True)

    # Create initial files and commit
    (repo / "spec.md").write_text("# Spec")
    (repo / "plan.md").write_text("# Plan")

    # Create command file structure
    cmd_dir = repo / ".claude" / "commands" / "tanren"
    cmd_dir.mkdir(parents=True)
    (cmd_dir / "do-task.md").write_text("# do-task")

    # Create spec folder
    spec_dir = repo / "specs" / "test-spec"
    spec_dir.mkdir(parents=True)

    for cmd in [
        ["git", "-C", str(repo), "add", "-A"],
        ["git", "-C", str(repo), "commit", "-m", "initial"],
    ]:
        subprocess.run(cmd, capture_output=True, check=True)

    return repo


class TestPreflightIntegration:
    @pytest.mark.asyncio
    async def test_dirty_tree_cleaned(self, git_repo: Path):
        """Create dirty files, run preflight, verify stashed."""
        # Make tree dirty
        (git_repo / "dirty.txt").write_text("uncommitted")

        spec_folder = git_repo / "specs" / "test-spec"
        result = await run_preflight(git_repo, "test-branch", spec_folder, "do-task")

        assert result.passed is True
        assert any("Stashed" in r for r in result.repairs)

        # Verify tree is clean after preflight
        proc = await asyncio.create_subprocess_exec(
            "git",
            "-C",
            str(git_repo),
            "status",
            "--porcelain",
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        stdout, _ = await proc.communicate()
        assert not stdout.decode().strip()

    @pytest.mark.asyncio
    async def test_file_snapshots_real(self, git_repo: Path):
        """Verify file snapshots on real files."""
        spec_folder = git_repo / "specs" / "test-spec"
        result = await run_preflight(git_repo, "test-branch", spec_folder, "do-task")

        assert result.passed is True
        assert "spec.md" in result.file_hashes
        assert "plan.md" in result.file_hashes
        assert result.file_backups["spec.md"] == "# Spec"

    @pytest.mark.asyncio
    async def test_clean_repo_passes(self, git_repo: Path):
        """Clean repo should pass with no repairs."""
        spec_folder = git_repo / "specs" / "test-spec"
        result = await run_preflight(git_repo, "test-branch", spec_folder, "do-task")

        assert result.passed is True
        assert result.repairs == []
