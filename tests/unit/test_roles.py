"""Tests for roles module."""

from tanren_core.roles import AgentTool, AuthMode, Cli, RoleMapping


class TestAgentTool:
    def test_defaults(self):
        tool = AgentTool(cli=Cli.CLAUDE)
        assert tool.cli == "claude"
        assert tool.model is None
        assert tool.auth == "api_key"

    def test_full(self):
        tool = AgentTool(
            cli=Cli.OPENCODE,
            model="custom-model",
            endpoint="https://llm.example.com/v1",
            auth=AuthMode.OAUTH,
            cli_path="/usr/local/bin/opencode",
        )
        assert tool.cli == "opencode"
        assert tool.model == "custom-model"


class TestRoleMapping:
    def test_resolve_default(self):
        mapping = RoleMapping(default=AgentTool(cli=Cli.CLAUDE))
        tool = mapping.resolve("implementation")
        assert tool.cli == "claude"

    def test_resolve_specific_role(self):
        mapping = RoleMapping(
            default=AgentTool(cli=Cli.CLAUDE),
            implementation=AgentTool(cli=Cli.OPENCODE, model="custom"),
        )
        tool = mapping.resolve("implementation")
        assert tool.cli == "opencode"
        assert tool.model == "custom"

    def test_resolve_missing_role_falls_back(self):
        mapping = RoleMapping(
            default=AgentTool(cli=Cli.CLAUDE),
            audit=AgentTool(cli=Cli.CODEX),
        )
        tool = mapping.resolve("conversation")
        assert tool.cli == "claude"

    def test_all_roles(self):
        mapping = RoleMapping(
            default=AgentTool(cli=Cli.CLAUDE),
            conversation=AgentTool(cli=Cli.CLAUDE, model="opus"),
            implementation=AgentTool(cli=Cli.OPENCODE),
            audit=AgentTool(cli=Cli.CODEX),
            feedback=AgentTool(cli=Cli.CLAUDE, model="sonnet"),
            conflict_resolution=AgentTool(cli=Cli.CLAUDE, model="opus"),
        )
        assert mapping.resolve("audit").cli == "codex"
        assert mapping.resolve("feedback").model == "sonnet"


class TestRequiredClis:
    def test_single_cli(self):
        mapping = RoleMapping(default=AgentTool(cli=Cli.CLAUDE))
        assert mapping.required_clis() == frozenset({Cli.CLAUDE})

    def test_multi_cli(self):
        mapping = RoleMapping(
            default=AgentTool(cli=Cli.CLAUDE),
            implementation=AgentTool(cli=Cli.OPENCODE),
            audit=AgentTool(cli=Cli.CODEX),
        )
        assert mapping.required_clis() == frozenset({Cli.CLAUDE, Cli.OPENCODE, Cli.CODEX})

    def test_bash_excluded(self):
        mapping = RoleMapping(
            default=AgentTool(cli=Cli.CLAUDE),
            feedback=AgentTool(cli=Cli.BASH),
        )
        assert mapping.required_clis() == frozenset({Cli.CLAUDE})

    def test_deduplication(self):
        mapping = RoleMapping(
            default=AgentTool(cli=Cli.CLAUDE),
            conversation=AgentTool(cli=Cli.CLAUDE, model="opus"),
            feedback=AgentTool(cli=Cli.CLAUDE, model="sonnet"),
        )
        assert mapping.required_clis() == frozenset({Cli.CLAUDE})

    def test_all_roles_multi_cli(self):
        mapping = RoleMapping(
            default=AgentTool(cli=Cli.CLAUDE),
            conversation=AgentTool(cli=Cli.CLAUDE, model="opus"),
            implementation=AgentTool(cli=Cli.OPENCODE),
            audit=AgentTool(cli=Cli.CODEX),
            feedback=AgentTool(cli=Cli.CLAUDE, model="sonnet"),
            conflict_resolution=AgentTool(cli=Cli.CODEX),
        )
        assert mapping.required_clis() == frozenset({Cli.CLAUDE, Cli.OPENCODE, Cli.CODEX})
