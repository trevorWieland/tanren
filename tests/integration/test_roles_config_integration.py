"""Integration tests for roles_config YAML loading and parsing."""

from typing import TYPE_CHECKING

import pytest

from tanren_core.roles import RoleMapping
from tanren_core.roles_config import load_roles_config
from tanren_core.schemas import AuthMode, Cli

if TYPE_CHECKING:
    from pathlib import Path


class TestLoadRolesConfig:
    def test_full_roles_yml(self, tmp_path: Path):
        """Load a complete roles.yml with all roles specified."""
        cfg = tmp_path / "roles.yml"
        cfg.write_text(
            "agents:\n"
            "  default:\n"
            "    cli: claude\n"
            "    auth: api_key\n"
            "    model: sonnet-4\n"
            "  implementation:\n"
            "    cli: opencode\n"
            "    auth: api_key\n"
            "    model: sonnet-4\n"
            "  audit:\n"
            "    cli: codex\n"
            "    auth: api_key\n"
            "    model: o3\n"
        )
        mapping = load_roles_config(cfg)
        assert isinstance(mapping, RoleMapping)
        assert mapping.default.cli == Cli.CLAUDE
        assert mapping.default.model == "sonnet-4"
        assert mapping.implementation is not None
        assert mapping.implementation.cli == Cli.OPENCODE
        assert mapping.audit is not None
        assert mapping.audit.cli == Cli.CODEX

    def test_minimal_roles_yml(self, tmp_path: Path):
        """Only default agent — all other roles should be None."""
        cfg = tmp_path / "roles.yml"
        cfg.write_text(
            "agents:\n  default:\n    cli: claude\n    auth: api_key\n    model: sonnet-4\n"
        )
        mapping = load_roles_config(cfg)
        assert mapping.default.cli == Cli.CLAUDE
        assert mapping.conversation is None
        assert mapping.implementation is None
        assert mapping.audit is None
        assert mapping.feedback is None
        assert mapping.conflict_resolution is None

    def test_required_clis(self, tmp_path: Path):
        """required_clis() returns correct CLI set excluding bash."""
        cfg = tmp_path / "roles.yml"
        cfg.write_text(
            "agents:\n"
            "  default:\n"
            "    cli: claude\n"
            "    auth: api_key\n"
            "    model: sonnet-4\n"
            "  audit:\n"
            "    cli: codex\n"
            "    auth: api_key\n"
            "    model: o3\n"
        )
        mapping = load_roles_config(cfg)
        clis = mapping.required_clis()
        assert Cli.CLAUDE in clis
        assert Cli.CODEX in clis
        assert Cli.BASH not in clis

    def test_missing_file_raises(self, tmp_path: Path):
        with pytest.raises(FileNotFoundError, match="Roles config not found"):
            load_roles_config(tmp_path / "nonexistent.yml")

    def test_malformed_yaml_not_mapping(self, tmp_path: Path):
        cfg = tmp_path / "roles.yml"
        cfg.write_text("just a string\n")
        with pytest.raises(TypeError, match="expected a mapping"):
            load_roles_config(cfg)

    def test_empty_yaml(self, tmp_path: Path):
        cfg = tmp_path / "roles.yml"
        cfg.write_text("")
        with pytest.raises(TypeError, match="expected a mapping"):
            load_roles_config(cfg)

    def test_missing_agents_section(self, tmp_path: Path):
        cfg = tmp_path / "roles.yml"
        cfg.write_text("version: 1\n")
        with pytest.raises(TypeError, match="missing required 'agents' section"):
            load_roles_config(cfg)

    def test_missing_default_agent(self, tmp_path: Path):
        cfg = tmp_path / "roles.yml"
        cfg.write_text(
            "agents:\n"
            "  implementation:\n"
            "    cli: opencode\n"
            "    auth: api_key\n"
            "    model: sonnet-4\n"
        )
        with pytest.raises(TypeError, match=r"agents\.default must be a mapping"):
            load_roles_config(cfg)

    def test_invalid_cli_value(self, tmp_path: Path):
        cfg = tmp_path / "roles.yml"
        cfg.write_text(
            "agents:\n  default:\n    cli: invalid_cli\n    auth: api_key\n    model: sonnet-4\n"
        )
        with pytest.raises(ValueError, match="Invalid CLI value"):
            load_roles_config(cfg)

    def test_missing_cli_field(self, tmp_path: Path):
        cfg = tmp_path / "roles.yml"
        cfg.write_text("agents:\n  default:\n    auth: api_key\n    model: sonnet-4\n")
        with pytest.raises(ValueError, match="missing required 'cli' field"):
            load_roles_config(cfg)

    def test_hyphenated_role_key(self, tmp_path: Path):
        """conflict-resolution (hyphenated) maps to conflict_resolution."""
        cfg = tmp_path / "roles.yml"
        cfg.write_text(
            "agents:\n"
            "  default:\n"
            "    cli: claude\n"
            "    auth: api_key\n"
            "    model: sonnet-4\n"
            "  conflict-resolution:\n"
            "    cli: opencode\n"
            "    auth: oauth\n"
            "    model: opus-4\n"
        )
        mapping = load_roles_config(cfg)
        assert mapping.conflict_resolution is not None
        assert mapping.conflict_resolution.cli == Cli.OPENCODE
        assert mapping.conflict_resolution.auth == AuthMode.OAUTH

    def test_resolve_fallback_to_default(self, tmp_path: Path):
        """resolve() falls back to default for unconfigured roles."""
        cfg = tmp_path / "roles.yml"
        cfg.write_text(
            "agents:\n  default:\n    cli: claude\n    auth: api_key\n    model: sonnet-4\n"
        )
        mapping = load_roles_config(cfg)
        tool = mapping.resolve("implementation")
        assert tool.cli == Cli.CLAUDE  # falls back to default

    def test_optional_endpoint_and_cli_path(self, tmp_path: Path):
        """endpoint and cli_path are optional fields."""
        cfg = tmp_path / "roles.yml"
        cfg.write_text(
            "agents:\n"
            "  default:\n"
            "    cli: claude\n"
            "    auth: api_key\n"
            "    model: sonnet-4\n"
            "    endpoint: https://api.example.com\n"
            "    cli_path: /usr/local/bin/claude\n"
        )
        mapping = load_roles_config(cfg)
        assert mapping.default.endpoint == "https://api.example.com"
        assert mapping.default.cli_path == "/usr/local/bin/claude"
