"""Tests for config module."""

import os

import pytest

from worker_manager.config import Config


class TestConfig:
    def test_requires_ipc_dir(self, monkeypatch: object):
        monkeypatch.delenv("WM_IPC_DIR", raising=False)
        with pytest.raises(ValueError, match="WM_IPC_DIR"):
            Config.from_env()

    def test_defaults_with_ipc_dir(self, monkeypatch: object):
        monkeypatch.setenv("WM_IPC_DIR", "/tmp/test-ipc")
        config = Config.from_env()
        assert config.poll_interval == 5.0
        assert config.heartbeat_interval == 30.0
        assert config.max_opencode == 1
        assert config.max_codex == 1
        assert config.max_gate == 3
        assert config.opencode_path == "opencode"
        assert config.codex_path == "codex"
        assert config.commands_dir == ".claude/commands/tanren"

    def test_ipc_dir_expanded(self, monkeypatch: object):
        monkeypatch.setenv("WM_IPC_DIR", "~/test-ipc")
        config = Config.from_env()
        assert "~" not in config.ipc_dir

    def test_data_dir_expanded(self, monkeypatch: object):
        monkeypatch.setenv("WM_IPC_DIR", "/tmp/test-ipc")
        config = Config.from_env()
        assert "~" not in config.data_dir

    def test_env_override(self, monkeypatch: object):
        monkeypatch.setattr(
            os,
            "environ",
            {
                **os.environ,
                "WM_IPC_DIR": "/tmp/test-ipc",
                "WM_POLL_INTERVAL": "10.0",
                "WM_MAX_GATE": "5",
                "WM_OPENCODE_PATH": "/usr/local/bin/opencode",
            },
        )
        config = Config.from_env()
        assert config.poll_interval == 10.0
        assert config.max_gate == 5
        assert config.opencode_path == "/usr/local/bin/opencode"

    def test_claude_path_default(self, monkeypatch: object):
        monkeypatch.setenv("WM_IPC_DIR", "/tmp/test-ipc")
        config = Config.from_env()
        assert config.claude_path == "claude"

    def test_roles_config_path_default(self, monkeypatch: object):
        monkeypatch.setenv("WM_IPC_DIR", "/tmp/test-ipc")
        config = Config.from_env()
        assert config.roles_config_path is None

    def test_roles_config_path_from_env(self, monkeypatch: object):
        monkeypatch.setattr(
            os,
            "environ",
            {
                **os.environ,
                "WM_IPC_DIR": "/tmp/test-ipc",
                "WM_ROLES_CONFIG_PATH": "/etc/tanren/roles.yml",
            },
        )
        config = Config.from_env()
        assert config.roles_config_path == "/etc/tanren/roles.yml"

    def test_worktree_registry_in_data_dir(self, monkeypatch: object):
        monkeypatch.setenv("WM_IPC_DIR", "/tmp/test-ipc")
        config = Config.from_env()
        assert config.worktree_registry_path.startswith(config.data_dir)
        assert config.worktree_registry_path.endswith("worktrees.json")
