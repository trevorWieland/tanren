"""Tests for config module."""

import os

from worker_manager.config import Config


class TestConfig:
    def test_defaults(self):
        config = Config.from_env()
        assert config.poll_interval == 5.0
        assert config.heartbeat_interval == 30.0
        assert config.max_opencode == 1
        assert config.max_codex == 1
        assert config.max_gate == 3
        assert config.opencode_path == "opencode"
        assert config.codex_path == "codex"
        assert config.commands_dir == ".claude/commands/tanren"

    def test_ipc_dir_expanded(self):
        config = Config.from_env()
        assert "~" not in config.ipc_dir

    def test_data_dir_expanded(self):
        config = Config.from_env()
        assert "~" not in config.data_dir

    def test_env_override(self, monkeypatch: object):
        monkeypatch.setattr(os, "environ", {
            **os.environ,
            "WM_POLL_INTERVAL": "10.0",
            "WM_MAX_GATE": "5",
            "WM_OPENCODE_PATH": "/usr/local/bin/opencode",
        })
        config = Config.from_env()
        assert config.poll_interval == 10.0
        assert config.max_gate == 5
        assert config.opencode_path == "/usr/local/bin/opencode"

    def test_worktree_registry_in_data_dir(self):
        config = Config.from_env()
        assert config.worktree_registry_path.startswith(config.data_dir)
        assert config.worktree_registry_path.endswith("worktrees.json")
