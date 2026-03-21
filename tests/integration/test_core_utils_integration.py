"""Integration tests for env/validator, env/provision, and preflight modules."""

import asyncio
import subprocess
from typing import TYPE_CHECKING

import pytest

from tanren_core.env.provision import provision_worktree_env
from tanren_core.env.schema import EnvBlock, OptionalEnvVar, RequiredEnvVar
from tanren_core.env.validator import VarStatus, validate_env
from tanren_core.preflight import SNAPSHOT_FILES, run_preflight

if TYPE_CHECKING:
    from pathlib import Path

# ---------------------------------------------------------------------------
# env/validator.py
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
class TestValidateEnv:
    async def test_validate_required_pattern_mismatch(self, monkeypatch) -> None:
        """Required var exists but doesn't match pattern -> PATTERN_MISMATCH."""
        monkeypatch.delenv("TEST_API_KEY_INTEG", raising=False)
        env_block = EnvBlock(
            required=[
                RequiredEnvVar(
                    key="TEST_API_KEY_INTEG", pattern=r"^sk-", hint="Must start with sk-"
                ),
            ],
        )
        merged = {"TEST_API_KEY_INTEG": "wrong-prefix-value"}
        source_map = {"TEST_API_KEY_INTEG": ".env"}

        report = await validate_env(env_block, merged, source_map)

        assert not report.passed
        assert len(report.required_results) == 1
        assert report.required_results[0].status == VarStatus.PATTERN_MISMATCH
        assert report.required_results[0].key == "TEST_API_KEY_INTEG"

    async def test_validate_optional_missing_with_default(self, monkeypatch) -> None:
        """Optional var missing, has default -> DEFAULTED status, default applied."""
        monkeypatch.delenv("OPT_LOG_LEVEL", raising=False)
        env_block = EnvBlock(
            optional=[
                OptionalEnvVar(key="OPT_LOG_LEVEL", default="INFO", description="Log level"),
            ],
        )
        merged: dict[str, str] = {}
        source_map: dict[str, str] = {}

        report = await validate_env(env_block, merged, source_map)

        assert report.passed
        assert len(report.optional_results) == 1
        assert report.optional_results[0].status == VarStatus.DEFAULTED
        # Default should be injected into merged_env
        assert merged["OPT_LOG_LEVEL"] == "INFO"
        assert source_map["OPT_LOG_LEVEL"] == "default"

    async def test_validate_optional_missing_no_default(self, monkeypatch) -> None:
        """Optional var missing, no default -> MISSING but still passes."""
        monkeypatch.delenv("OPT_EXTRA_FLAG", raising=False)
        env_block = EnvBlock(
            optional=[
                OptionalEnvVar(key="OPT_EXTRA_FLAG", description="Extra flag"),
            ],
        )
        merged: dict[str, str] = {}
        source_map: dict[str, str] = {}

        report = await validate_env(env_block, merged, source_map)

        assert report.passed
        assert len(report.optional_results) == 1
        assert report.optional_results[0].status == VarStatus.MISSING
        assert "OPT_EXTRA_FLAG" not in merged

    async def test_validate_empty_required(self, monkeypatch) -> None:
        """Required var exists but empty string -> EMPTY status."""
        monkeypatch.delenv("EMPTY_REQ_VAR", raising=False)
        env_block = EnvBlock(
            required=[
                RequiredEnvVar(key="EMPTY_REQ_VAR", description="Must be non-empty"),
            ],
        )
        merged = {"EMPTY_REQ_VAR": ""}
        source_map = {"EMPTY_REQ_VAR": ".env"}

        report = await validate_env(env_block, merged, source_map)

        assert not report.passed
        assert len(report.required_results) == 1
        assert report.required_results[0].status == VarStatus.EMPTY


# ---------------------------------------------------------------------------
# preflight.py
# ---------------------------------------------------------------------------


@pytest.fixture
def preflight_git_repo(tmp_path: Path) -> Path:
    """Create a real git repo with branch, commit, and command structure."""
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
    for fname in SNAPSHOT_FILES:
        (repo / fname).write_text(f"# {fname} content")

    # Create command file structure for agent phases
    cmd_dir = repo / ".claude" / "commands" / "tanren"
    cmd_dir.mkdir(parents=True)
    for phase in ("do-task", "audit-task", "run-demo", "audit-spec", "investigate"):
        (cmd_dir / f"{phase}.md").write_text(f"# {phase}")

    # Create spec folder
    spec_dir = repo / "specs" / "test-spec"
    spec_dir.mkdir(parents=True)

    for cmd in [
        ["git", "-C", str(repo), "add", "-A"],
        ["git", "-C", str(repo), "commit", "-m", "initial"],
    ]:
        subprocess.run(cmd, capture_output=True, check=True)

    return repo


class TestPreflightSnapshots:
    @pytest.mark.asyncio
    async def test_snapshot_captures_all_existing_files(self, preflight_git_repo: Path) -> None:
        """Preflight should snapshot all SNAPSHOT_FILES that exist."""
        spec_folder = preflight_git_repo / "specs" / "test-spec"
        result = await run_preflight(preflight_git_repo, "test-branch", spec_folder, "do-task")

        assert result.passed
        for fname in SNAPSHOT_FILES:
            assert fname in result.file_hashes
            assert fname in result.file_backups
            assert result.file_backups[fname] == f"# {fname} content"

    @pytest.mark.asyncio
    async def test_snapshot_skips_missing_files(self, preflight_git_repo: Path) -> None:
        """Files in SNAPSHOT_FILES that don't exist should be skipped."""
        # Remove pyproject.toml so it's missing
        (preflight_git_repo / "pyproject.toml").unlink()
        await asyncio.to_thread(
            subprocess.run,
            ["git", "-C", str(preflight_git_repo), "add", "-A"],
            capture_output=True,
            check=True,
        )
        await asyncio.to_thread(
            subprocess.run,
            ["git", "-C", str(preflight_git_repo), "commit", "-m", "remove pyproject"],
            capture_output=True,
            check=True,
        )

        spec_folder = preflight_git_repo / "specs" / "test-spec"
        result = await run_preflight(preflight_git_repo, "test-branch", spec_folder, "do-task")

        assert result.passed
        assert "pyproject.toml" not in result.file_hashes
        assert "pyproject.toml" not in result.file_backups

    @pytest.mark.asyncio
    async def test_clears_agent_status_files(self, preflight_git_repo: Path) -> None:
        """Preflight should remove .agent-status and findings files from spec folder."""
        spec_folder = preflight_git_repo / "specs" / "test-spec"
        # Create status files and commit them so the tree stays clean
        (spec_folder / ".agent-status").write_text("do-task-status: complete")
        (spec_folder / "audit-findings.json").write_text("{}")
        (spec_folder / "investigation-report.json").write_text("{}")
        for cmd in [
            ["git", "-C", str(preflight_git_repo), "add", "-A"],
            ["git", "-C", str(preflight_git_repo), "commit", "-m", "add status files"],
        ]:
            await asyncio.to_thread(subprocess.run, cmd, capture_output=True, check=True)

        result = await run_preflight(preflight_git_repo, "test-branch", spec_folder, "do-task")

        assert result.passed
        assert not (spec_folder / ".agent-status").exists()
        assert not (spec_folder / "audit-findings.json").exists()
        assert not (spec_folder / "investigation-report.json").exists()
        assert any("Cleared" in r for r in result.repairs)

    @pytest.mark.asyncio
    async def test_missing_command_file_fails(self, preflight_git_repo: Path) -> None:
        """Missing command file for agent phase should fail preflight."""
        spec_folder = preflight_git_repo / "specs" / "test-spec"
        # Remove the do-task command file and commit so tree stays clean
        (preflight_git_repo / ".claude" / "commands" / "tanren" / "do-task.md").unlink()
        for cmd in [
            ["git", "-C", str(preflight_git_repo), "add", "-A"],
            ["git", "-C", str(preflight_git_repo), "commit", "-m", "remove do-task"],
        ]:
            await asyncio.to_thread(subprocess.run, cmd, capture_output=True, check=True)

        result = await run_preflight(preflight_git_repo, "test-branch", spec_folder, "do-task")

        assert not result.passed
        assert result.error is not None
        assert "do-task.md" in result.error

    @pytest.mark.asyncio
    async def test_gate_phase_skips_command_file_check(self, preflight_git_repo: Path) -> None:
        """Gate phase should not check for command files."""
        spec_folder = preflight_git_repo / "specs" / "test-spec"

        result = await run_preflight(preflight_git_repo, "test-branch", spec_folder, "gate")

        assert result.passed

    @pytest.mark.asyncio
    async def test_wrong_branch_gets_repaired(self, preflight_git_repo: Path) -> None:
        """If on wrong branch, preflight should switch and report repair."""
        # Create and switch to a different branch
        await asyncio.to_thread(
            subprocess.run,
            ["git", "-C", str(preflight_git_repo), "checkout", "-b", "wrong-branch"],
            capture_output=True,
            check=True,
        )

        spec_folder = preflight_git_repo / "specs" / "test-spec"
        result = await run_preflight(preflight_git_repo, "test-branch", spec_folder, "gate")

        assert result.passed
        assert any("Switched branch" in r for r in result.repairs)


# ---------------------------------------------------------------------------
# env/provision.py
# ---------------------------------------------------------------------------


def _write_tanren_yml(worktree: Path, content: str) -> None:
    """Write a tanren.yml file into the given worktree directory."""
    (worktree / "tanren.yml").write_text(content)


class TestProvisionWorktreeEnv:
    def test_provision_with_full_env_block(self, tmp_path: Path, monkeypatch) -> None:
        """Provision resolves required + optional vars from os.environ and .env."""
        worktree = tmp_path / "worktree"
        worktree.mkdir()
        project_dir = tmp_path / "project"
        project_dir.mkdir()

        _write_tanren_yml(
            worktree,
            "version: '1'\n"
            "profile: default\n"
            "installed: '2026-01-01'\n"
            "env:\n"
            "  required:\n"
            "    - key: DATABASE_URL\n"
            "    - key: API_KEY\n"
            "  optional:\n"
            "    - key: LOG_LEVEL\n"
            "      default: INFO\n",
        )

        # Set env vars so they can be resolved
        monkeypatch.setenv("DATABASE_URL", "postgres://localhost/test")
        monkeypatch.setenv("API_KEY", "sk-test-123")
        monkeypatch.setenv("LOG_LEVEL", "DEBUG")

        count = provision_worktree_env(worktree, project_dir)

        assert count == 3
        dotenv_path = worktree / ".env"
        assert dotenv_path.exists()
        content = dotenv_path.read_text()
        assert "DATABASE_URL=postgres://localhost/test" in content
        assert "API_KEY=sk-test-123" in content
        assert "LOG_LEVEL=DEBUG" in content

    def test_provision_with_secrets_dir(self, tmp_path: Path, monkeypatch) -> None:
        """Provision loads secrets from secrets.env in secrets_dir."""
        worktree = tmp_path / "worktree"
        worktree.mkdir()
        project_dir = tmp_path / "project"
        project_dir.mkdir()
        secrets_dir = tmp_path / "secrets"
        secrets_dir.mkdir()

        # Write a secrets.env file
        (secrets_dir / "secrets.env").write_text("SECRET_TOKEN=s3cr3t\n")

        _write_tanren_yml(
            worktree,
            "version: '1'\n"
            "profile: default\n"
            "installed: '2026-01-01'\n"
            "env:\n"
            "  required:\n"
            "    - key: SECRET_TOKEN\n",
        )

        # Ensure SECRET_TOKEN is NOT in os.environ so secrets.env is the source
        monkeypatch.delenv("SECRET_TOKEN", raising=False)

        count = provision_worktree_env(worktree, project_dir, secrets_dir=secrets_dir)

        assert count == 1
        dotenv_path = worktree / ".env"
        assert dotenv_path.exists()
        content = dotenv_path.read_text()
        assert "SECRET_TOKEN=s3cr3t" in content

    def test_provision_no_tanren_yml_returns_zero(self, tmp_path: Path) -> None:
        """No tanren.yml file -> returns 0, no .env written."""
        worktree = tmp_path / "worktree"
        worktree.mkdir()
        project_dir = tmp_path / "project"
        project_dir.mkdir()

        count = provision_worktree_env(worktree, project_dir)

        assert count == 0
        assert not (worktree / ".env").exists()

    def test_provision_empty_env_block_returns_zero(self, tmp_path: Path) -> None:
        """tanren.yml exists but no env section -> returns 0."""
        worktree = tmp_path / "worktree"
        worktree.mkdir()
        project_dir = tmp_path / "project"
        project_dir.mkdir()

        _write_tanren_yml(
            worktree,
            "version: '1'\nprofile: default\ninstalled: '2026-01-01'\n",
        )

        count = provision_worktree_env(worktree, project_dir)

        assert count == 0
        assert not (worktree / ".env").exists()

    def test_provision_env_block_with_no_vars_returns_zero(self, tmp_path: Path) -> None:
        """tanren.yml has env block but no required or optional vars -> returns 0."""
        worktree = tmp_path / "worktree"
        worktree.mkdir()
        project_dir = tmp_path / "project"
        project_dir.mkdir()

        _write_tanren_yml(
            worktree,
            "version: '1'\n"
            "profile: default\n"
            "installed: '2026-01-01'\n"
            "env:\n"
            "  required: []\n"
            "  optional: []\n",
        )

        count = provision_worktree_env(worktree, project_dir)

        assert count == 0

    def test_provision_unresolvable_vars_not_written(self, tmp_path: Path, monkeypatch) -> None:
        """Vars that cannot be resolved are not written to .env."""
        worktree = tmp_path / "worktree"
        worktree.mkdir()
        project_dir = tmp_path / "project"
        project_dir.mkdir()

        _write_tanren_yml(
            worktree,
            "version: '1'\n"
            "profile: default\n"
            "installed: '2026-01-01'\n"
            "env:\n"
            "  required:\n"
            "    - key: KNOWN_VAR\n"
            "    - key: UNKNOWN_VAR\n",
        )

        monkeypatch.setenv("KNOWN_VAR", "value1")
        monkeypatch.delenv("UNKNOWN_VAR", raising=False)

        count = provision_worktree_env(worktree, project_dir)

        assert count == 1
        content = (worktree / ".env").read_text()
        assert "KNOWN_VAR=value1" in content
        assert "UNKNOWN_VAR" not in content

    def test_provision_reads_from_project_dotenv(self, tmp_path: Path, monkeypatch) -> None:
        """Provision resolves vars from the project-local .env file."""
        worktree = tmp_path / "worktree"
        worktree.mkdir()
        project_dir = tmp_path / "project"
        project_dir.mkdir()

        # Write a .env in the project dir
        (project_dir / ".env").write_text("PROJECT_VAR=from_project\n")

        _write_tanren_yml(
            worktree,
            "version: '1'\n"
            "profile: default\n"
            "installed: '2026-01-01'\n"
            "env:\n"
            "  required:\n"
            "    - key: PROJECT_VAR\n",
        )

        monkeypatch.delenv("PROJECT_VAR", raising=False)

        count = provision_worktree_env(worktree, project_dir)

        assert count == 1
        content = (worktree / ".env").read_text()
        assert "PROJECT_VAR=from_project" in content
