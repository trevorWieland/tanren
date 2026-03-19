"""Integration tests for MCP config injection flow."""

import json
from unittest.mock import AsyncMock

import pytest

from tanren_core.adapters.git_workspace import GitAuthConfig, GitWorkspaceManager
from tanren_core.adapters.remote_types import RemoteResult, WorkspacePath
from tanren_core.env.environment_schema import (
    EnvironmentProfile,
    McpServerConfig,
    parse_environment_profiles,
)

# ---------------------------------------------------------------------------
# Schema: tanren.yml -> McpServerConfig round-trip
# ---------------------------------------------------------------------------


class TestMcpSchemaRoundTrip:
    def test_tanren_yml_with_mcp_parses(self):
        data = {
            "environment": {
                "default": {
                    "type": "remote",
                    "mcp": {
                        "context7": {
                            "url": "https://mcp.context7.com/mcp",
                            "headers": {"CONTEXT_API_KEY": "MCP_CONTEXT7_KEY"},
                        },
                        "other-server": {
                            "url": "https://other.example.com/sse",
                        },
                    },
                }
            }
        }
        profiles = parse_environment_profiles(data)
        profile = profiles["default"]

        assert len(profile.mcp) == 2
        assert profile.mcp["context7"].url == "https://mcp.context7.com/mcp"
        assert profile.mcp["context7"].headers == {"CONTEXT_API_KEY": "MCP_CONTEXT7_KEY"}
        assert profile.mcp["other-server"].headers == {}

    def test_invalid_server_name_rejected(self):
        with pytest.raises(ValueError, match="must match"):
            EnvironmentProfile(
                name="test",
                mcp={"has.dot": McpServerConfig(url="https://example.com")},
            )

    def test_empty_mcp_default(self):
        profiles = parse_environment_profiles({})
        assert profiles["default"].mcp == {}


# ---------------------------------------------------------------------------
# Rendering: all three CLI config formats
# ---------------------------------------------------------------------------


class TestMcpConfigRendering:
    @pytest.mark.asyncio
    async def test_all_three_configs_written(self):
        conn = AsyncMock()
        conn.run = AsyncMock(return_value=RemoteResult(exit_code=0, stdout="", stderr=""))
        conn.upload_content = AsyncMock()

        mgr = GitWorkspaceManager(GitAuthConfig())
        workspace = WorkspacePath(path="/workspace/proj", project="proj", branch="main")
        servers = {
            "ctx7": McpServerConfig(
                url="https://mcp.context7.com/mcp",
                headers={"CONTEXT_API_KEY": "MCP_CONTEXT7_KEY"},
            ),
        }

        await mgr.inject_mcp_config(conn, workspace, servers)

        assert conn.upload_content.call_count == 3
        uploaded = {c.args[1]: c.args[0] for c in conn.upload_content.call_args_list}

        # Claude
        claude = json.loads(uploaded["/workspace/proj/.mcp.json"])
        assert claude["mcpServers"]["ctx7"]["type"] == "http"
        assert claude["mcpServers"]["ctx7"]["headers"]["CONTEXT_API_KEY"] == "${MCP_CONTEXT7_KEY}"

        # Codex
        codex = uploaded["/workspace/proj/.codex/config.toml"]
        assert "[mcp_servers.ctx7]" in codex
        assert 'CONTEXT_API_KEY = "MCP_CONTEXT7_KEY"' in codex

        # OpenCode
        opencode = json.loads(uploaded["/workspace/proj/opencode.json"])
        assert opencode["mcp"]["ctx7"]["type"] == "remote"
        assert opencode["mcp"]["ctx7"]["headers"]["CONTEXT_API_KEY"] == "{env:MCP_CONTEXT7_KEY}"

    @pytest.mark.asyncio
    async def test_empty_servers_no_writes(self):
        conn = AsyncMock()
        conn.run = AsyncMock(return_value=RemoteResult(exit_code=0, stdout="", stderr=""))
        conn.upload_content = AsyncMock()

        mgr = GitWorkspaceManager(GitAuthConfig())
        workspace = WorkspacePath(path="/workspace/proj", project="proj", branch="main")

        await mgr.inject_mcp_config(conn, workspace, {})

        conn.upload_content.assert_not_called()
