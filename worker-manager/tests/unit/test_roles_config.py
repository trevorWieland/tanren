"""Tests for roles_config module."""

import pytest

from worker_manager.roles_config import load_roles_config


class TestLoadRolesConfig:
    def test_load_valid_config(self, tmp_path):
        config_file = tmp_path / "roles.yml"
        config_file.write_text("""
agents:
  default:
    cli: claude
    model: claude-sonnet-4-20250514
    auth: oauth
  implementation:
    cli: opencode
    model: custom-model
    endpoint: https://llm.example.com/v1
    auth: api_key
  audit:
    cli: codex
    auth: api_key
""")
        mapping = load_roles_config(config_file)
        assert mapping.default.cli == "claude"
        assert mapping.default.model == "claude-sonnet-4-20250514"
        assert mapping.implementation.cli == "opencode"
        assert mapping.audit.cli == "codex"

    def test_load_minimal_config(self, tmp_path):
        config_file = tmp_path / "roles.yml"
        config_file.write_text("""
agents:
  default:
    cli: claude
""")
        mapping = load_roles_config(config_file)
        assert mapping.default.cli == "claude"
        assert mapping.implementation is None

    def test_load_missing_file(self, tmp_path):
        mapping = load_roles_config(tmp_path / "nonexistent.yml")
        assert mapping.default.cli == "claude"

    def test_invalid_cli_raises(self, tmp_path):
        config_file = tmp_path / "roles.yml"
        config_file.write_text("""
agents:
  default:
    cli: invalid-tool
""")
        with pytest.raises(ValueError, match="Invalid CLI value"):
            load_roles_config(config_file)

    def test_hyphenated_role_key(self, tmp_path):
        config_file = tmp_path / "roles.yml"
        config_file.write_text("""
agents:
  default:
    cli: claude
  conflict-resolution:
    cli: claude
    model: opus
""")
        mapping = load_roles_config(config_file)
        assert mapping.conflict_resolution is not None
        assert mapping.conflict_resolution.model == "opus"
