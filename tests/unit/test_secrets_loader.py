"""Tests for secrets loader."""

import os
from typing import TYPE_CHECKING

from tanren_core.adapters.remote_types import SecretBundle
from tanren_core.schemas import Cli
from tanren_core.secrets import SecretConfig, SecretLoader

if TYPE_CHECKING:
    from pathlib import Path

_ALL_CLIS = frozenset({Cli.CLAUDE, Cli.CODEX, Cli.OPENCODE})


class TestLoadDeveloper:
    def test_reads_from_secrets_file(self, tmp_path: Path):
        secrets_file = tmp_path / "secrets.env"
        secrets_file.write_text("API_KEY=sk-abc123\nDB_URL=postgres://localhost\n")
        config = SecretConfig(developer_secrets_path=str(secrets_file))
        loader = SecretLoader(config, required_clis=_ALL_CLIS)

        result = loader.load_developer()

        assert result == {"API_KEY": "sk-abc123", "DB_URL": "postgres://localhost"}

    def test_returns_empty_dict_when_file_missing(self, tmp_path: Path):
        config = SecretConfig(developer_secrets_path=str(tmp_path / "nonexistent.env"))
        loader = SecretLoader(config, required_clis=_ALL_CLIS)

        result = loader.load_developer()

        assert result == {}


class TestLoadInfrastructure:
    def test_reads_from_env_vars(self, monkeypatch):
        monkeypatch.setenv("GIT_TOKEN", "ghp_abc123")
        config = SecretConfig(infrastructure_env_vars=("GIT_TOKEN",))
        loader = SecretLoader(config, required_clis=_ALL_CLIS)

        result = loader.load_infrastructure()

        assert result == {"GIT_TOKEN": "ghp_abc123"}


class TestLoadCredentialFiles:
    def test_reads_claude_credentials(self, tmp_path: Path):
        secrets_file = tmp_path / "secrets.env"
        secrets_file.write_text("")
        (tmp_path / "claude_credentials.json").write_text('{"token": "abc"}')
        loader = SecretLoader(
            SecretConfig(developer_secrets_path=str(secrets_file)),
            required_clis=frozenset({Cli.CLAUDE}),
        )

        result = loader.load_credential_files()

        assert result == {"CLAUDE_CREDENTIALS_JSON": '{"token": "abc"}'}

    def test_reads_codex_auth(self, tmp_path: Path):
        secrets_file = tmp_path / "secrets.env"
        secrets_file.write_text("")
        (tmp_path / "codex_auth.json").write_text('{"session": "xyz"}')
        loader = SecretLoader(
            SecretConfig(developer_secrets_path=str(secrets_file)),
            required_clis=frozenset({Cli.CODEX}),
        )

        result = loader.load_credential_files()

        assert result == {"CODEX_AUTH_JSON": '{"session": "xyz"}'}

    def test_reads_both_files(self, tmp_path: Path):
        secrets_file = tmp_path / "secrets.env"
        secrets_file.write_text("")
        (tmp_path / "claude_credentials.json").write_text('{"token": "abc"}')
        (tmp_path / "codex_auth.json").write_text('{"session": "xyz"}')
        loader = SecretLoader(
            SecretConfig(developer_secrets_path=str(secrets_file)),
            required_clis=frozenset({Cli.CLAUDE, Cli.CODEX}),
        )

        result = loader.load_credential_files()

        assert result == {
            "CLAUDE_CREDENTIALS_JSON": '{"token": "abc"}',
            "CODEX_AUTH_JSON": '{"session": "xyz"}',
        }

    def test_returns_empty_when_no_files(self, tmp_path: Path):
        secrets_file = tmp_path / "secrets.env"
        secrets_file.write_text("")
        loader = SecretLoader(
            SecretConfig(developer_secrets_path=str(secrets_file)),
            required_clis=_ALL_CLIS,
        )

        result = loader.load_credential_files()

        assert result == {}

    def test_skips_empty_files(self, tmp_path: Path):
        secrets_file = tmp_path / "secrets.env"
        secrets_file.write_text("")
        (tmp_path / "claude_credentials.json").write_text("   \n  ")
        loader = SecretLoader(
            SecretConfig(developer_secrets_path=str(secrets_file)),
            required_clis=frozenset({Cli.CLAUDE}),
        )

        result = loader.load_credential_files()

        assert result == {}


class TestRequiredClisFiltering:
    def test_only_claude_file_loaded(self, tmp_path: Path):
        secrets_file = tmp_path / "secrets.env"
        secrets_file.write_text("")
        (tmp_path / "claude_credentials.json").write_text('{"token": "abc"}')
        (tmp_path / "codex_auth.json").write_text('{"session": "xyz"}')
        loader = SecretLoader(
            SecretConfig(developer_secrets_path=str(secrets_file)),
            required_clis=frozenset({Cli.CLAUDE}),
        )

        result = loader.load_credential_files()

        assert result == {"CLAUDE_CREDENTIALS_JSON": '{"token": "abc"}'}

    def test_only_codex_file_loaded(self, tmp_path: Path):
        secrets_file = tmp_path / "secrets.env"
        secrets_file.write_text("")
        (tmp_path / "claude_credentials.json").write_text('{"token": "abc"}')
        (tmp_path / "codex_auth.json").write_text('{"session": "xyz"}')
        loader = SecretLoader(
            SecretConfig(developer_secrets_path=str(secrets_file)),
            required_clis=frozenset({Cli.CODEX}),
        )

        result = loader.load_credential_files()

        assert result == {"CODEX_AUTH_JSON": '{"session": "xyz"}'}

    def test_opencode_has_no_credential_file(self, tmp_path: Path):
        secrets_file = tmp_path / "secrets.env"
        secrets_file.write_text("")
        loader = SecretLoader(
            SecretConfig(developer_secrets_path=str(secrets_file)),
            required_clis=frozenset({Cli.OPENCODE}),
        )

        result = loader.load_credential_files()

        assert result == {}


class TestBuildBundle:
    def test_combines_all_sources(self, tmp_path: Path, monkeypatch):
        secrets_file = tmp_path / "secrets.env"
        secrets_file.write_text("DEV_SECRET=dev_val\n")
        (tmp_path / "claude_credentials.json").write_text('{"token": "abc"}')
        monkeypatch.setenv("GIT_TOKEN", "ghp_xyz")
        config = SecretConfig(
            developer_secrets_path=str(secrets_file),
            infrastructure_env_vars=("GIT_TOKEN",),
        )
        loader = SecretLoader(config, required_clis=frozenset({Cli.CLAUDE}))

        bundle = loader.build_bundle(project_secrets={"PROJ_KEY": "proj_val"})

        assert isinstance(bundle, SecretBundle)
        assert bundle.developer["DEV_SECRET"] == "dev_val"
        assert bundle.developer["CLAUDE_CREDENTIALS_JSON"] == '{"token": "abc"}'
        assert bundle.project == {"PROJ_KEY": "proj_val"}
        assert bundle.infrastructure == {"GIT_TOKEN": "ghp_xyz"}


class TestDefaultPath:
    def test_xdg_default_used_when_env_not_set(self, monkeypatch):
        monkeypatch.delenv("XDG_CONFIG_HOME", raising=False)
        config = SecretConfig()

        assert "tanren" in config.developer_secrets_path
        assert "secrets.env" in config.developer_secrets_path


class TestAutoload:
    def test_constructor_does_not_mutate_process_env(self, tmp_path: Path, monkeypatch):
        secrets_file = tmp_path / "secrets.env"
        secrets_file.write_text("HCLOUD_TOKEN=from-file\n")
        monkeypatch.delenv("HCLOUD_TOKEN", raising=False)

        SecretLoader(
            SecretConfig(developer_secrets_path=str(secrets_file)), required_clis=_ALL_CLIS
        )

        assert os.environ.get("HCLOUD_TOKEN") is None

    def test_autoloads_developer_secrets_into_process_env(self, tmp_path: Path, monkeypatch):
        secrets_file = tmp_path / "secrets.env"
        secrets_file.write_text("HCLOUD_TOKEN=from-file\n")
        monkeypatch.delenv("HCLOUD_TOKEN", raising=False)

        SecretLoader(
            SecretConfig(developer_secrets_path=str(secrets_file)), required_clis=_ALL_CLIS
        ).autoload_into_env()

        assert os.environ.get("HCLOUD_TOKEN") == "from-file"

    def test_autoload_does_not_override_existing_env(self, tmp_path: Path, monkeypatch):
        secrets_file = tmp_path / "secrets.env"
        secrets_file.write_text("HCLOUD_TOKEN=from-file\n")
        monkeypatch.setenv("HCLOUD_TOKEN", "explicit")

        SecretLoader(
            SecretConfig(developer_secrets_path=str(secrets_file)), required_clis=_ALL_CLIS
        ).autoload_into_env()

        assert os.environ.get("HCLOUD_TOKEN") == "explicit"
