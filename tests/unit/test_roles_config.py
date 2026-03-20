"""Tests for roles_config module."""

import pytest

from tanren_core.roles_config import load_roles_config


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
    model: o3
    auth: api_key
""")
        mapping = load_roles_config(config_file)
        assert mapping.default.cli == "claude"
        assert mapping.default.model == "claude-sonnet-4-20250514"
        assert mapping.implementation is not None
        assert mapping.audit is not None
        assert mapping.implementation.cli == "opencode"
        assert mapping.audit.cli == "codex"

    def test_load_minimal_config(self, tmp_path):
        config_file = tmp_path / "roles.yml"
        config_file.write_text("""
agents:
  default:
    cli: claude
    model: claude-sonnet-4-20250514
    auth: api_key
""")
        mapping = load_roles_config(config_file)
        assert mapping.default.cli == "claude"
        assert mapping.implementation is None

    def test_load_missing_file(self, tmp_path):
        with pytest.raises(FileNotFoundError, match="Roles config not found"):
            load_roles_config(tmp_path / "nonexistent.yml")

    def test_invalid_cli_raises(self, tmp_path):
        config_file = tmp_path / "roles.yml"
        config_file.write_text("""
agents:
  default:
    cli: invalid-tool
    auth: api_key
""")
        with pytest.raises(ValueError, match="Invalid CLI value"):
            load_roles_config(config_file)

    def test_invalid_yaml_raises(self, tmp_path):
        config_file = tmp_path / "roles.yml"
        config_file.write_text("just a string")
        with pytest.raises(TypeError, match="expected a mapping"):
            load_roles_config(config_file)

    def test_missing_agents_section_raises(self, tmp_path):
        config_file = tmp_path / "roles.yml"
        config_file.write_text("something_else:\n  key: value\n")
        with pytest.raises(TypeError, match="missing required 'agents' section"):
            load_roles_config(config_file)

    def test_missing_default_raises(self, tmp_path):
        config_file = tmp_path / "roles.yml"
        config_file.write_text(
            "agents:\n  implementation:\n    cli: opencode\n    model: m1\n    auth: api_key\n"
        )
        with pytest.raises(TypeError, match=r"agents\.default must be a mapping"):
            load_roles_config(config_file)

    def test_missing_cli_raises(self, tmp_path):
        config_file = tmp_path / "roles.yml"
        config_file.write_text("agents:\n  default:\n    model: sonnet\n    auth: api_key\n")
        with pytest.raises(ValueError, match="missing required 'cli' field"):
            load_roles_config(config_file)

    def test_missing_model_raises(self, tmp_path):
        config_file = tmp_path / "roles.yml"
        config_file.write_text("agents:\n  default:\n    cli: claude\n    auth: api_key\n")
        with pytest.raises(ValueError, match="missing required 'model' field"):
            load_roles_config(config_file)

    def test_missing_auth_raises(self, tmp_path):
        config_file = tmp_path / "roles.yml"
        config_file.write_text("agents:\n  default:\n    cli: claude\n    model: sonnet\n")
        with pytest.raises(ValueError, match="missing required 'auth' field"):
            load_roles_config(config_file)

    def test_hyphenated_role_key(self, tmp_path):
        config_file = tmp_path / "roles.yml"
        config_file.write_text("""
agents:
  default:
    cli: claude
    model: sonnet
    auth: oauth
  conflict-resolution:
    cli: claude
    model: opus
    auth: api_key
""")
        mapping = load_roles_config(config_file)
        assert mapping.conflict_resolution is not None
        assert mapping.conflict_resolution.model == "opus"
