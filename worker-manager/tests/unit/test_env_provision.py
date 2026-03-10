"""Tests for env provision module."""

import os
from pathlib import Path
from unittest.mock import patch

from worker_manager.env.provision import provision_worktree_env


def _write_tanren_yml(path: Path, content: str) -> None:
    (path / "tanren.yml").write_text(content)


def _write_dotenv(path: Path, content: str) -> None:
    (path / ".env").write_text(content)


class TestProvisionWorktreeEnv:
    def test_no_tanren_yml_returns_zero(self, tmp_path: Path):
        worktree = tmp_path / "wt"
        worktree.mkdir()
        project = tmp_path / "proj"
        project.mkdir()

        count = provision_worktree_env(worktree, project)
        assert count == 0
        assert not (worktree / ".env").exists()

    def test_tanren_yml_without_env_block_returns_zero(self, tmp_path: Path):
        worktree = tmp_path / "wt"
        worktree.mkdir()
        project = tmp_path / "proj"
        project.mkdir()

        _write_tanren_yml(
            worktree,
            "version: 0.1.0\nprofile: default\ninstalled: 2026-01-01\n",
        )

        count = provision_worktree_env(worktree, project)
        assert count == 0
        assert not (worktree / ".env").exists()

    def test_writes_required_vars_from_main_repo(self, tmp_path: Path):
        worktree = tmp_path / "wt"
        worktree.mkdir()
        project = tmp_path / "proj"
        project.mkdir()

        _write_tanren_yml(
            worktree,
            "version: 0.1.0\n"
            "profile: default\n"
            "installed: 2026-01-01\n"
            "env:\n"
            "  required:\n"
            "    - key: API_KEY\n"
            "    - key: BASE_URL\n",
        )
        _write_dotenv(project, "API_KEY=sk-123\nBASE_URL=https://example.com\n")

        count = provision_worktree_env(worktree, project)
        assert count == 2

        dotenv = (worktree / ".env").read_text()
        assert "API_KEY=sk-123" in dotenv
        assert "BASE_URL=https://example.com" in dotenv

    def test_only_defined_keys_written(self, tmp_path: Path):
        """Only tanren.yml-defined keys are written — no leaking unrelated vars."""
        worktree = tmp_path / "wt"
        worktree.mkdir()
        project = tmp_path / "proj"
        project.mkdir()

        _write_tanren_yml(
            worktree,
            "version: 0.1.0\n"
            "profile: default\n"
            "installed: 2026-01-01\n"
            "env:\n"
            "  required:\n"
            "    - key: WANTED\n",
        )
        _write_dotenv(project, "WANTED=yes\nUNWANTED=leaked\n")

        count = provision_worktree_env(worktree, project)
        assert count == 1

        dotenv = (worktree / ".env").read_text()
        assert "WANTED=yes" in dotenv
        assert "UNWANTED" not in dotenv

    def test_os_environ_picked_up_for_defined_keys(self, tmp_path: Path):
        worktree = tmp_path / "wt"
        worktree.mkdir()
        project = tmp_path / "proj"
        project.mkdir()

        _write_tanren_yml(
            worktree,
            "version: 0.1.0\n"
            "profile: default\n"
            "installed: 2026-01-01\n"
            "env:\n"
            "  required:\n"
            "    - key: FROM_ENV\n",
        )

        with patch.dict(os.environ, {"FROM_ENV": "from-environ"}, clear=False):
            count = provision_worktree_env(worktree, project)

        assert count == 1
        dotenv = (worktree / ".env").read_text()
        assert "FROM_ENV=from-environ" in dotenv

    def test_optional_vars_not_written_when_unresolved(self, tmp_path: Path):
        """Optional vars with defaults are NOT written if not resolved."""
        worktree = tmp_path / "wt"
        worktree.mkdir()
        project = tmp_path / "proj"
        project.mkdir()

        _write_tanren_yml(
            worktree,
            "version: 0.1.0\n"
            "profile: default\n"
            "installed: 2026-01-01\n"
            "env:\n"
            "  required:\n"
            "    - key: API_KEY\n"
            "  optional:\n"
            "    - key: LOG_LEVEL\n"
            '      default: "INFO"\n',
        )
        _write_dotenv(project, "API_KEY=sk-123\n")

        count = provision_worktree_env(worktree, project)
        assert count == 1

        dotenv = (worktree / ".env").read_text()
        assert "API_KEY=sk-123" in dotenv
        assert "LOG_LEVEL" not in dotenv

    def test_secrets_resolved(self, tmp_path: Path):
        worktree = tmp_path / "wt"
        worktree.mkdir()
        project = tmp_path / "proj"
        project.mkdir()
        sd = tmp_path / "secrets"
        sd.mkdir()
        (sd / "secrets.env").write_text("SECRET_KEY=secret-val\n")

        _write_tanren_yml(
            worktree,
            "version: 0.1.0\n"
            "profile: default\n"
            "installed: 2026-01-01\n"
            "env:\n"
            "  required:\n"
            "    - key: SECRET_KEY\n",
        )

        count = provision_worktree_env(worktree, project, secrets_dir=sd)
        assert count == 1

        dotenv = (worktree / ".env").read_text()
        assert "SECRET_KEY=secret-val" in dotenv

    def test_empty_env_block_returns_zero(self, tmp_path: Path):
        worktree = tmp_path / "wt"
        worktree.mkdir()
        project = tmp_path / "proj"
        project.mkdir()

        _write_tanren_yml(
            worktree,
            "version: 0.1.0\nprofile: default\ninstalled: 2026-01-01\nenv:\n  on_missing: error\n",
        )

        count = provision_worktree_env(worktree, project)
        assert count == 0
        assert not (worktree / ".env").exists()
