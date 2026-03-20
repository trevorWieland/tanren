"""Tests for env CLI subcommands."""

import json
from typing import TYPE_CHECKING

from typer.testing import CliRunner

from tanren_cli.env_cli import env, secret

if TYPE_CHECKING:
    from pathlib import Path


class TestEnvCheck:
    def test_pass_with_environment_block_present(self, tmp_path: Path, monkeypatch):
        monkeypatch.chdir(tmp_path)
        monkeypatch.setenv("MY_KEY", "hello")
        (tmp_path / "tanren.yml").write_text(
            "version: 0.1.0\n"
            "profile: default\n"
            "installed: 2026-01-01\n"
            "environment:\n"
            "  ci:\n"
            "    type: docker\n"
            "env:\n"
            "  required:\n"
            "    - key: MY_KEY\n"
        )
        runner = CliRunner()
        result = runner.invoke(env, ["check"])
        assert result.exit_code == 0
        assert "PASSED" in result.output

    def test_no_tanren_yml(self, tmp_path: Path, monkeypatch):
        monkeypatch.chdir(tmp_path)
        runner = CliRunner()
        result = runner.invoke(env, ["check"])
        assert result.exit_code != 0

    def test_no_env_block(self, tmp_path: Path, monkeypatch):
        monkeypatch.chdir(tmp_path)
        (tmp_path / "tanren.yml").write_text(
            "version: 0.1.0\nprofile: default\ninstalled: 2026-01-01\n"
        )
        runner = CliRunner()
        result = runner.invoke(env, ["check"])
        assert result.exit_code == 0
        assert "No env requirements" in result.output

    def test_pass(self, tmp_path: Path, monkeypatch):
        monkeypatch.chdir(tmp_path)
        monkeypatch.setenv("MY_KEY", "hello")
        (tmp_path / "tanren.yml").write_text(
            "version: 0.1.0\nprofile: default\ninstalled: 2026-01-01\n"
            "env:\n  required:\n    - key: MY_KEY\n"
        )
        runner = CliRunner()
        result = runner.invoke(env, ["check"])
        assert result.exit_code == 0
        assert "PASSED" in result.output

    def test_fail_missing(self, tmp_path: Path, monkeypatch):
        monkeypatch.chdir(tmp_path)
        monkeypatch.delenv("NONEXISTENT_KEY_XYZ", raising=False)
        (tmp_path / "tanren.yml").write_text(
            "version: 0.1.0\nprofile: default\ninstalled: 2026-01-01\n"
            "env:\n  required:\n    - key: NONEXISTENT_KEY_XYZ\n"
        )
        runner = CliRunner()
        result = runner.invoke(env, ["check"])
        assert result.exit_code == 1
        assert "FAILED" in result.output

    def test_json_output(self, tmp_path: Path, monkeypatch):
        monkeypatch.chdir(tmp_path)
        monkeypatch.setenv("MY_KEY", "val")
        (tmp_path / "tanren.yml").write_text(
            "version: 0.1.0\nprofile: default\ninstalled: 2026-01-01\n"
            "env:\n  required:\n    - key: MY_KEY\n"
        )
        runner = CliRunner()
        result = runner.invoke(env, ["check", "--json"])
        assert result.exit_code == 0

        data = json.loads(result.output)
        assert data["passed"] is True

    def test_verbose(self, tmp_path: Path, monkeypatch):
        monkeypatch.chdir(tmp_path)
        monkeypatch.setenv("MY_KEY", "val")
        (tmp_path / "tanren.yml").write_text(
            "version: 0.1.0\nprofile: default\ninstalled: 2026-01-01\n"
            "env:\n  required:\n    - key: MY_KEY\n"
        )
        runner = CliRunner()
        result = runner.invoke(env, ["check", "--verbose"])
        assert result.exit_code == 0
        assert "MY_KEY" in result.output

    def test_check_all(self, tmp_path: Path, monkeypatch):
        monkeypatch.chdir(tmp_path)
        monkeypatch.setenv("K", "v")
        sub = tmp_path / "sub"
        sub.mkdir()
        (sub / "tanren.yml").write_text(
            "version: 0.1.0\nprofile: default\ninstalled: 2026-01-01\n"
            "env:\n  required:\n    - key: K\n"
        )
        runner = CliRunner()
        result = runner.invoke(env, ["check", "--all"])
        assert result.exit_code == 0


class TestEnvInit:
    def test_scaffolds_env_block_when_environment_present(self, tmp_path: Path, monkeypatch):
        monkeypatch.chdir(tmp_path)
        (tmp_path / "tanren.yml").write_text(
            "version: 0.1.0\n"
            "profile: default\n"
            "installed: 2026-01-01\n"
            "environment:\n"
            "  default:\n"
            "    type: local\n"
        )
        (tmp_path / ".env.example").write_text("API_KEY=xxx\n")
        runner = CliRunner()
        result = runner.invoke(env, ["init"])
        assert result.exit_code == 0
        content = (tmp_path / "tanren.yml").read_text()
        assert "environment:" in content
        assert "env:" in content
        assert "API_KEY" in content

    def test_scaffolds_env_block(self, tmp_path: Path, monkeypatch):
        monkeypatch.chdir(tmp_path)
        (tmp_path / "tanren.yml").write_text(
            "version: 0.1.0\nprofile: default\ninstalled: 2026-01-01\n"
        )
        (tmp_path / ".env.example").write_text("API_KEY=xxx\nDB_URL=yyy\n")
        runner = CliRunner()
        result = runner.invoke(env, ["init"])
        assert result.exit_code == 0
        content = (tmp_path / "tanren.yml").read_text()
        assert "env:" in content
        assert "API_KEY" in content
        assert "DB_URL" in content

    def test_no_tanren_yml(self, tmp_path: Path, monkeypatch):
        monkeypatch.chdir(tmp_path)
        runner = CliRunner()
        result = runner.invoke(env, ["init"])
        assert result.exit_code != 0

    def test_env_block_already_exists(self, tmp_path: Path, monkeypatch):
        monkeypatch.chdir(tmp_path)
        (tmp_path / "tanren.yml").write_text(
            "version: 0.1.0\nprofile: default\ninstalled: 2026-01-01\n"
            "env:\n  required:\n    - key: X\n"
        )
        runner = CliRunner()
        result = runner.invoke(env, ["init"])
        assert result.exit_code != 0
        assert "already has an env block" in result.output


class TestSecretSet:
    def test_set_secret(self, tmp_path: Path, monkeypatch):
        sd = tmp_path / "secrets"
        monkeypatch.setattr(
            "tanren_cli.env_cli.set_secret",
            lambda key, value: (
                sd.mkdir(parents=True, exist_ok=True),
                (sd / "secrets.env").write_text(f'{key}="{value}"\n'),
                sd / "secrets.env",
            )[-1],
        )
        runner = CliRunner()
        result = runner.invoke(secret, ["set", "MY_KEY", "my_value"])
        assert result.exit_code == 0
        assert "MY_KEY" in result.output


class TestSecretList:
    def test_list_empty(self, tmp_path: Path, monkeypatch):
        monkeypatch.setattr(
            "tanren_cli.env_cli.list_secrets",
            lambda: [],
        )
        runner = CliRunner()
        result = runner.invoke(secret, ["list"])
        assert result.exit_code == 0
        assert "No secrets" in result.output

    def test_list_secrets(self, tmp_path: Path, monkeypatch):
        monkeypatch.setattr(
            "tanren_cli.env_cli.list_secrets",
            lambda: [("API_KEY", "sk-o...")],
        )
        runner = CliRunner()
        result = runner.invoke(secret, ["list"])
        assert result.exit_code == 0
        assert "API_KEY" in result.output
        assert "sk-o..." in result.output
