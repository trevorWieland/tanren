"""Tests for config module."""

import os
from typing import TYPE_CHECKING

import pytest

from tanren_core.config import (
    _REQUIRED_KEYS,  # noqa: PLC2701 — testing private implementation
    Config,
    ConfigSource,
    DotenvConfigSource,
    load_config_env,
)

if TYPE_CHECKING:
    from pathlib import Path

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

_ALL_REQUIRED_ENV = {
    "WM_IPC_DIR": "/tmp/test-ipc",
    "WM_GITHUB_DIR": "/tmp/github",
    "WM_DATA_DIR": "/tmp/data",
    "WM_COMMANDS_DIR": ".claude/commands/tanren",
    "WM_POLL_INTERVAL": "5.0",
    "WM_HEARTBEAT_INTERVAL": "30.0",
    "WM_OPENCODE_PATH": "opencode",
    "WM_CODEX_PATH": "codex",
    "WM_CLAUDE_PATH": "claude",
    "WM_MAX_OPENCODE": "1",
    "WM_MAX_CODEX": "1",
    "WM_MAX_GATE": "3",
    "WM_WORKTREE_REGISTRY_PATH": "/tmp/data/worktrees.json",
    "WM_ROLES_CONFIG_PATH": "/tmp/roles.yml",
}


def _set_all_required(monkeypatch):
    """Set all required WM_* env vars via monkeypatch."""
    for key, value in _ALL_REQUIRED_ENV.items():
        monkeypatch.setenv(key, value)


class _DictSource:
    """Trivial ConfigSource for testing."""

    def __init__(self, values: dict[str, str]) -> None:
        self._values = values

    def load(self) -> dict[str, str]:
        return dict(self._values)


# ---------------------------------------------------------------------------
# TestDotenvConfigSource
# ---------------------------------------------------------------------------


class TestDotenvConfigSource:
    def test_loads_values_from_file(self, tmp_path: Path):
        env_file = tmp_path / "tanren.env"
        env_file.write_text("WM_IPC_DIR=/tmp/ipc\nWM_GITHUB_DIR=/tmp/gh\n")
        source = DotenvConfigSource(path=env_file)
        values = source.load()
        assert values == {"WM_IPC_DIR": "/tmp/ipc", "WM_GITHUB_DIR": "/tmp/gh"}

    def test_returns_empty_when_file_missing(self, tmp_path: Path):
        source = DotenvConfigSource(path=tmp_path / "nonexistent.env")
        assert source.load() == {}

    def test_respects_xdg_config_home(self, tmp_path: Path, monkeypatch):
        monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path))
        config_dir = tmp_path / "tanren"
        config_dir.mkdir()
        env_file = config_dir / "tanren.env"
        env_file.write_text("WM_IPC_DIR=/xdg/ipc\n")
        source = DotenvConfigSource()
        values = source.load()
        assert values == {"WM_IPC_DIR": "/xdg/ipc"}

    def test_expands_tilde_in_xdg_config_home(self, monkeypatch):
        monkeypatch.setenv("XDG_CONFIG_HOME", "~/custom-config")
        source = DotenvConfigSource()
        assert "~" not in str(source._path)


# ---------------------------------------------------------------------------
# TestLoadConfigEnv
# ---------------------------------------------------------------------------


class TestLoadConfigEnv:
    def test_populates_os_environ(self, monkeypatch):
        monkeypatch.delenv("WM_IPC_DIR", raising=False)
        source = _DictSource({"WM_IPC_DIR": "/from/source"})
        load_config_env(source=source)
        assert os.environ["WM_IPC_DIR"] == "/from/source"
        # Clean up: load_config_env writes directly to os.environ
        monkeypatch.delenv("WM_IPC_DIR")

    def test_env_var_overrides_source(self, monkeypatch):
        monkeypatch.setenv("WM_IPC_DIR", "/from/env")
        source = _DictSource({"WM_IPC_DIR": "/from/source"})
        load_config_env(source=source)
        assert os.environ["WM_IPC_DIR"] == "/from/env"

    def test_ignores_non_wm_keys(self, monkeypatch):
        """load_config_env only injects known WM_* keys, not arbitrary keys."""
        monkeypatch.delenv("LEAKED_SECRET", raising=False)
        monkeypatch.delenv("WM_IPC_DIR", raising=False)
        source = _DictSource({"LEAKED_SECRET": "oops", "WM_IPC_DIR": "/from/source"})
        load_config_env(source=source)
        assert os.environ.get("LEAKED_SECRET") is None
        assert os.environ["WM_IPC_DIR"] == "/from/source"
        monkeypatch.delenv("WM_IPC_DIR")


# ---------------------------------------------------------------------------
# TestConfigFromEnvWithSources
# ---------------------------------------------------------------------------


class TestConfigFromEnvWithSources:
    @pytest.fixture(autouse=True)
    def _clear_wm_env(self, monkeypatch):
        """Ensure no WM_* env vars leak into source-based tests."""
        for key in (*_REQUIRED_KEYS, "WM_ROLES_CONFIG_PATH", "WM_EVENTS_DB", "WM_REMOTE_CONFIG"):
            monkeypatch.delenv(key, raising=False)

    def test_resolves_all_required_from_source(self):
        source = _DictSource(_ALL_REQUIRED_ENV)
        config = Config.from_env(sources=[source])
        assert config.ipc_dir == "/tmp/test-ipc"
        assert config.github_dir == "/tmp/github"
        assert config.poll_interval == pytest.approx(5.0)
        assert config.max_gate == 3

    def test_env_overrides_source(self, monkeypatch):
        source = _DictSource(_ALL_REQUIRED_ENV)
        monkeypatch.setenv("WM_POLL_INTERVAL", "99.0")
        config = Config.from_env(sources=[source])
        assert config.poll_interval == pytest.approx(99.0)

    def test_raises_on_missing_required(self):
        source = _DictSource({"WM_IPC_DIR": "/tmp/ipc"})
        with pytest.raises(ValueError, match="Missing required config"):
            Config.from_env(sources=[source])

    def test_optional_keys_default_to_none(self):
        source = _DictSource(_ALL_REQUIRED_ENV)
        config = Config.from_env(sources=[source])
        assert config.events_db is None
        assert config.remote_config_path is None


# ---------------------------------------------------------------------------
# TestConfig (existing — updated for zero-defaults)
# ---------------------------------------------------------------------------


class TestConfig:
    def test_raises_on_missing_required_keys(self, monkeypatch):
        """from_env() with no sources and no env vars raises for all required keys."""
        for key in _REQUIRED_KEYS:
            monkeypatch.delenv(key, raising=False)
        with pytest.raises(ValueError, match="Missing required config"):
            Config.from_env()

    def test_ipc_dir_expanded(self, monkeypatch):
        _set_all_required(monkeypatch)
        monkeypatch.setenv("WM_IPC_DIR", "~/test-ipc")
        config = Config.from_env()
        assert "~" not in config.ipc_dir

    def test_data_dir_expanded(self, monkeypatch):
        _set_all_required(monkeypatch)
        monkeypatch.setenv("WM_DATA_DIR", "~/data")
        config = Config.from_env()
        assert "~" not in config.data_dir

    def test_env_override(self, monkeypatch):
        _set_all_required(monkeypatch)
        monkeypatch.setenv("WM_POLL_INTERVAL", "10.0")
        monkeypatch.setenv("WM_MAX_GATE", "5")
        monkeypatch.setenv("WM_OPENCODE_PATH", "/usr/local/bin/opencode")
        config = Config.from_env()
        assert config.poll_interval == pytest.approx(10.0)
        assert config.max_gate == 5
        assert config.opencode_path == "/usr/local/bin/opencode"

    def test_claude_path(self, monkeypatch):
        _set_all_required(monkeypatch)
        config = Config.from_env()
        assert config.claude_path == "claude"

    def test_roles_config_path_from_env(self, monkeypatch):
        _set_all_required(monkeypatch)
        monkeypatch.setenv("WM_ROLES_CONFIG_PATH", "/etc/tanren/roles.yml")
        config = Config.from_env()
        assert config.roles_config_path == "/etc/tanren/roles.yml"

    def test_worktree_registry_path(self, monkeypatch):
        _set_all_required(monkeypatch)
        config = Config.from_env()
        assert config.worktree_registry_path == "/tmp/data/worktrees.json"

    def test_worktree_registry_path_expanded(self, monkeypatch):
        _set_all_required(monkeypatch)
        monkeypatch.setenv("WM_WORKTREE_REGISTRY_PATH", "~/wt.json")
        config = Config.from_env()
        assert "~" not in config.worktree_registry_path

    def test_optional_paths_expanded(self, monkeypatch):
        _set_all_required(monkeypatch)
        monkeypatch.setenv("WM_ROLES_CONFIG_PATH", "~/roles.yml")
        monkeypatch.setenv("WM_EVENTS_DB", "~/events.db")
        monkeypatch.setenv("WM_REMOTE_CONFIG", "~/remote.yml")
        config = Config.from_env()
        assert config.roles_config_path is not None
        assert config.events_db is not None
        assert config.remote_config_path is not None
        assert "~" not in config.roles_config_path
        assert "~" not in config.events_db
        assert "~" not in config.remote_config_path

    def test_raises_on_empty_required_value(self, monkeypatch):
        _set_all_required(monkeypatch)
        monkeypatch.setenv("WM_IPC_DIR", "")
        with pytest.raises(ValueError, match="WM_IPC_DIR"):
            Config.from_env()

    def test_raises_on_whitespace_only_value(self, monkeypatch):
        _set_all_required(monkeypatch)
        monkeypatch.setenv("WM_IPC_DIR", "   ")
        with pytest.raises(ValueError, match="WM_IPC_DIR"):
            Config.from_env()

    def test_empty_optional_value_treated_as_none(self, monkeypatch):
        _set_all_required(monkeypatch)
        monkeypatch.setenv("WM_REMOTE_CONFIG", "")
        config = Config.from_env()
        assert config.remote_config_path is None

    def test_whitespace_optional_value_treated_as_none(self, monkeypatch):
        _set_all_required(monkeypatch)
        monkeypatch.setenv("WM_EVENTS_DB", "   ")
        config = Config.from_env()
        assert config.events_db is None

    def test_postgres_url_not_path_expanded(self, monkeypatch):
        _set_all_required(monkeypatch)
        monkeypatch.setenv("WM_EVENTS_DB", "postgresql://host/db")
        config = Config.from_env()
        assert config.events_db == "postgresql://host/db"

    def test_uppercase_postgres_url_not_path_expanded(self, monkeypatch):
        _set_all_required(monkeypatch)
        monkeypatch.setenv("WM_EVENTS_DB", "POSTGRESQL://host/db")
        config = Config.from_env()
        assert config.events_db == "POSTGRESQL://host/db"


# ---------------------------------------------------------------------------
# ConfigSource protocol check
# ---------------------------------------------------------------------------


class TestConfigSourceProtocol:
    def test_dotenv_source_satisfies_protocol(self):
        assert isinstance(DotenvConfigSource(), ConfigSource)

    def test_dict_source_satisfies_protocol(self):
        assert isinstance(_DictSource({}), ConfigSource)
