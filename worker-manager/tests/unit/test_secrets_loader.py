"""Tests for secrets loader."""

from pathlib import Path

from worker_manager.adapters.remote_types import SecretBundle
from worker_manager.secrets import SecretConfig, SecretLoader


class TestLoadDeveloper:
    def test_reads_from_secrets_file(self, tmp_path: Path):
        secrets_file = tmp_path / "secrets.env"
        secrets_file.write_text("API_KEY=sk-abc123\nDB_URL=postgres://localhost\n")
        config = SecretConfig(developer_secrets_path=str(secrets_file))
        loader = SecretLoader(config)

        result = loader.load_developer()

        assert result == {"API_KEY": "sk-abc123", "DB_URL": "postgres://localhost"}

    def test_returns_empty_dict_when_file_missing(self, tmp_path: Path):
        config = SecretConfig(developer_secrets_path=str(tmp_path / "nonexistent.env"))
        loader = SecretLoader(config)

        result = loader.load_developer()

        assert result == {}


class TestLoadInfrastructure:
    def test_reads_from_env_vars(self, monkeypatch):
        monkeypatch.setenv("GIT_TOKEN", "ghp_abc123")
        config = SecretConfig(infrastructure_env_vars=("GIT_TOKEN",))
        loader = SecretLoader(config)

        result = loader.load_infrastructure()

        assert result == {"GIT_TOKEN": "ghp_abc123"}


class TestBuildBundle:
    def test_combines_all_sources(self, tmp_path: Path, monkeypatch):
        secrets_file = tmp_path / "secrets.env"
        secrets_file.write_text("DEV_SECRET=dev_val\n")
        monkeypatch.setenv("GIT_TOKEN", "ghp_xyz")
        config = SecretConfig(
            developer_secrets_path=str(secrets_file),
            infrastructure_env_vars=("GIT_TOKEN",),
        )
        loader = SecretLoader(config)

        bundle = loader.build_bundle(project_secrets={"PROJ_KEY": "proj_val"})

        assert isinstance(bundle, SecretBundle)
        assert bundle.developer == {"DEV_SECRET": "dev_val"}
        assert bundle.project == {"PROJ_KEY": "proj_val"}
        assert bundle.infrastructure == {"GIT_TOKEN": "ghp_xyz"}


class TestDefaultPath:
    def test_xdg_default_used_when_env_not_set(self, monkeypatch):
        monkeypatch.delenv("XDG_CONFIG_HOME", raising=False)
        config = SecretConfig()

        assert "tanren" in config.developer_secrets_path
        assert "secrets.env" in config.developer_secrets_path
