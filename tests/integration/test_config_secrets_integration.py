"""Integration tests for config loading and secret management."""

import os
from pathlib import Path

import pytest

from tanren_core.adapters.remote_types import SecretBundle
from tanren_core.config import Config, DotenvConfigSource, load_config_env
from tanren_core.secrets import SecretConfig, SecretLoader

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

# All required WM_* environment variable values for a valid Config.
_REQUIRED_ENV = {
    "WM_IPC_DIR": "/tmp/ipc",
    "WM_GITHUB_DIR": "/tmp/github",
    "WM_DATA_DIR": "/tmp/data",
    "WM_COMMANDS_DIR": ".claude/commands/tanren",
    "WM_POLL_INTERVAL": "5",
    "WM_HEARTBEAT_INTERVAL": "30",
    "WM_OPENCODE_PATH": "opencode",
    "WM_CODEX_PATH": "codex",
    "WM_CLAUDE_PATH": "claude",
    "WM_MAX_OPENCODE": "1",
    "WM_MAX_CODEX": "1",
    "WM_MAX_GATE": "3",
    "WM_WORKTREE_REGISTRY_PATH": "/tmp/worktrees.json",
    "WM_ROLES_CONFIG_PATH": "/tmp/roles.yml",
}


class DictConfigSource:
    """Trivial ConfigSource backed by a plain dict."""

    def __init__(self, data: dict[str, str]) -> None:
        self._data = data

    def load(self) -> dict[str, str]:
        return dict(self._data)


# ---------------------------------------------------------------------------
# Config - DotenvConfigSource
# ---------------------------------------------------------------------------


class TestDotenvConfigSource:
    def test_loads_from_file(self, tmp_path: Path):
        env_file = tmp_path / "tanren.env"
        env_file.write_text("WM_IPC_DIR=/tmp/ipc\n")

        source = DotenvConfigSource(path=env_file)
        result = source.load()

        assert "WM_IPC_DIR" in result
        assert result["WM_IPC_DIR"] == "/tmp/ipc"

    def test_missing_file(self, tmp_path: Path):
        source = DotenvConfigSource(path=tmp_path / "nope.env")
        result = source.load()

        assert result == {}


# ---------------------------------------------------------------------------
# Config - load_config_env
# ---------------------------------------------------------------------------


class TestLoadConfigEnv:
    def test_sets_wm_vars(self, monkeypatch: pytest.MonkeyPatch):
        monkeypatch.delenv("WM_IPC_DIR", raising=False)
        source = DictConfigSource({"WM_IPC_DIR": "/tmp/ipc"})

        load_config_env(source)

        assert os.environ["WM_IPC_DIR"] == "/tmp/ipc"
        # Clean up via monkeypatch so env is restored after the test.
        monkeypatch.delenv("WM_IPC_DIR", raising=False)

    def test_skips_non_wm_keys(self, monkeypatch: pytest.MonkeyPatch):
        monkeypatch.delenv("OTHER_KEY", raising=False)
        source = DictConfigSource({"OTHER_KEY": "val"})

        load_config_env(source)

        assert "OTHER_KEY" not in os.environ

    def test_env_takes_precedence(self, monkeypatch: pytest.MonkeyPatch):
        monkeypatch.setenv("WM_IPC_DIR", "from_env")
        source = DictConfigSource({"WM_IPC_DIR": "from_source"})

        load_config_env(source)

        assert os.environ["WM_IPC_DIR"] == "from_env"


# ---------------------------------------------------------------------------
# Config - Config.from_env
# ---------------------------------------------------------------------------


class TestConfigFromEnv:
    def test_all_required(self, monkeypatch: pytest.MonkeyPatch):
        for key, value in _REQUIRED_ENV.items():
            monkeypatch.setenv(key, value)

        config = Config.from_env()

        assert config.ipc_dir == "/tmp/ipc"
        assert config.github_dir == "/tmp/github"
        assert config.data_dir == "/tmp/data"
        assert config.commands_dir == ".claude/commands/tanren"
        assert config.poll_interval == pytest.approx(5.0)
        assert config.heartbeat_interval == pytest.approx(30.0)
        assert config.opencode_path == "opencode"
        assert config.codex_path == "codex"
        assert config.claude_path == "claude"
        assert config.max_opencode == 1
        assert config.max_codex == 1
        assert config.max_gate == 3
        assert config.worktree_registry_path == "/tmp/worktrees.json"

    def test_missing_required(self, monkeypatch: pytest.MonkeyPatch):
        # Ensure none of the required vars are set.
        for key in _REQUIRED_ENV:
            monkeypatch.delenv(key, raising=False)

        with pytest.raises(ValueError, match="Missing required config"):
            Config.from_env()

    def test_tilde_expansion(self, monkeypatch: pytest.MonkeyPatch):
        env_with_tilde = {**_REQUIRED_ENV, "WM_IPC_DIR": "~/ipc"}
        for key, value in env_with_tilde.items():
            monkeypatch.setenv(key, value)

        config = Config.from_env()

        assert "~" not in config.ipc_dir
        assert config.ipc_dir.endswith("/ipc")

    def test_with_source(self, monkeypatch: pytest.MonkeyPatch):
        # Ensure env doesn't supply the values so the source is the only provider.
        for key in _REQUIRED_ENV:
            monkeypatch.delenv(key, raising=False)

        source = DictConfigSource(_REQUIRED_ENV)
        config = Config.from_env(sources=[source])

        assert config.ipc_dir == "/tmp/ipc"
        assert config.github_dir == "/tmp/github"

    def test_roles_config_path_tilde_expansion(self, monkeypatch: pytest.MonkeyPatch):
        for key, value in _REQUIRED_ENV.items():
            monkeypatch.setenv(key, value)
        monkeypatch.setenv("WM_ROLES_CONFIG_PATH", "~/roles.yml")

        config = Config.from_env()

        assert "~" not in config.roles_config_path
        assert config.roles_config_path.endswith("/roles.yml")


# ---------------------------------------------------------------------------
# Secrets - SecretLoader
# ---------------------------------------------------------------------------


class TestSecretLoaderDeveloper:
    def test_load_developer(self, tmp_path: Path):
        secrets_file = tmp_path / "secrets.env"
        secrets_file.write_text("API_KEY=sk-123\n")

        loader = SecretLoader(SecretConfig(developer_secrets_path=str(secrets_file)))
        result = loader.load_developer()

        assert result == {"API_KEY": "sk-123"}

    def test_load_developer_missing(self, tmp_path: Path):
        loader = SecretLoader(
            SecretConfig(developer_secrets_path=str(tmp_path / "nonexistent.env"))
        )
        result = loader.load_developer()

        assert result == {}


class TestSecretLoaderInfrastructure:
    def test_load_infrastructure(self, monkeypatch: pytest.MonkeyPatch):
        monkeypatch.setenv("GIT_TOKEN", "tok")

        loader = SecretLoader()
        result = loader.load_infrastructure()

        assert result == {"GIT_TOKEN": "tok"}

    def test_load_infrastructure_missing(self, monkeypatch: pytest.MonkeyPatch):
        monkeypatch.delenv("GIT_TOKEN", raising=False)

        loader = SecretLoader()
        result = loader.load_infrastructure()

        assert result == {}


class TestSecretLoaderBundle:
    def test_build_bundle(self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
        secrets_file = tmp_path / "secrets.env"
        secrets_file.write_text("DEV_KEY=dev_val\n")
        monkeypatch.setenv("GIT_TOKEN", "infra_tok")

        loader = SecretLoader(SecretConfig(developer_secrets_path=str(secrets_file)))
        bundle = loader.build_bundle(project_secrets={"PK": "v"})

        assert isinstance(bundle, SecretBundle)
        assert bundle.developer == {"DEV_KEY": "dev_val"}
        assert bundle.project == {"PK": "v"}
        assert bundle.infrastructure == {"GIT_TOKEN": "infra_tok"}


class TestSecretLoaderAutoload:
    def test_autoload_into_env(self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
        secrets_file = tmp_path / "secrets.env"
        secrets_file.write_text("AUTOLOAD_TEST_KEY=autoloaded\n")
        # Ensure the key is not already in the environment.
        monkeypatch.delenv("AUTOLOAD_TEST_KEY", raising=False)

        loader = SecretLoader(SecretConfig(developer_secrets_path=str(secrets_file)))
        loader.autoload_into_env()

        assert os.environ["AUTOLOAD_TEST_KEY"] == "autoloaded"
        # Clean up so the key doesn't leak into other tests.
        monkeypatch.delenv("AUTOLOAD_TEST_KEY", raising=False)
