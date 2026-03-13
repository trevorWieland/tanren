"""Tests for roles module."""

from tanren_core.roles import AgentTool, RoleMapping


class TestAgentTool:
    def test_defaults(self):
        tool = AgentTool(cli="claude")
        assert tool.cli == "claude"
        assert tool.model is None
        assert tool.auth == "api_key"

    def test_full(self):
        tool = AgentTool(
            cli="opencode",
            model="custom-model",
            endpoint="https://llm.example.com/v1",
            auth="oauth",
            cli_path="/usr/local/bin/opencode",
        )
        assert tool.cli == "opencode"
        assert tool.model == "custom-model"


class TestRoleMapping:
    def test_resolve_default(self):
        mapping = RoleMapping(default=AgentTool(cli="claude"))
        tool = mapping.resolve("implementation")
        assert tool.cli == "claude"

    def test_resolve_specific_role(self):
        mapping = RoleMapping(
            default=AgentTool(cli="claude"),
            implementation=AgentTool(cli="opencode", model="custom"),
        )
        tool = mapping.resolve("implementation")
        assert tool.cli == "opencode"
        assert tool.model == "custom"

    def test_resolve_missing_role_falls_back(self):
        mapping = RoleMapping(
            default=AgentTool(cli="claude"),
            audit=AgentTool(cli="codex"),
        )
        tool = mapping.resolve("conversation")
        assert tool.cli == "claude"

    def test_all_roles(self):
        mapping = RoleMapping(
            default=AgentTool(cli="claude"),
            conversation=AgentTool(cli="claude", model="opus"),
            implementation=AgentTool(cli="opencode"),
            audit=AgentTool(cli="codex"),
            feedback=AgentTool(cli="claude", model="sonnet"),
            conflict_resolution=AgentTool(cli="claude", model="opus"),
        )
        assert mapping.resolve("audit").cli == "codex"
        assert mapping.resolve("feedback").model == "sonnet"
