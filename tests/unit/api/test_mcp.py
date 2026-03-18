"""Tests for MCP server — tool registration, auth middleware, and tool execution."""

from __future__ import annotations

import json
from unittest.mock import AsyncMock, MagicMock

import pytest
from fastmcp import Client
from fastmcp.exceptions import ToolError

from tanren_api.mcp_auth import MCPApiKeyAuth
from tanren_api.mcp_server import mcp, set_services
from tanren_api.services import (
    ConfigService,
    DispatchService,
    EventsService,
    HealthService,
    RunService,
    VMService,
)
from tanren_api.settings import APISettings
from tanren_api.state import APIStateStore
from tanren_core.config import Config

TEST_API_KEY = "test-mcp-key-12345"


def _text(result) -> str:
    """Extract text from the first content block of a CallToolResult."""
    return result.content[0].text


@pytest.fixture(autouse=True)
def _clear_mcp_middleware():
    """Ensure a clean middleware stack for every test."""
    saved = list(mcp.middleware)
    mcp.middleware.clear()
    yield
    mcp.middleware.clear()
    mcp.middleware.extend(saved)


@pytest.fixture
def _seed_mcp_services(tmp_path, mock_execution_env, mock_vm_state_store):
    """Seed MCP service singletons with mocked dependencies for testing."""
    roles_yml = tmp_path / "roles.yml"
    roles_yml.write_text(
        "agents:\n  default:\n    cli: claude\n    model: sonnet\n    auth: oauth\n"
    )
    config = Config(
        ipc_dir=str(tmp_path / "ipc"),
        github_dir=str(tmp_path / "github"),
        data_dir=str(tmp_path / "data"),
        worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
        roles_config_path=str(roles_yml),
    )
    store = APIStateStore()
    settings = APISettings(api_key=TEST_API_KEY)

    set_services(
        health=HealthService(),
        dispatch=DispatchService(
            store=store, config=config, emitter=AsyncMock(), execution_env=mock_execution_env
        ),
        vm=VMService(
            store=store,
            config=config,
            execution_env=mock_execution_env,
            vm_state_store=mock_vm_state_store,
        ),
        run=RunService(store=store, config=config, execution_env=mock_execution_env),
        config=ConfigService(config),
        events=EventsService(settings, config),
    )


@pytest.fixture
def _mcp_auth():
    """Add auth middleware to the MCP server for this test scope."""
    mcp.add_middleware(MCPApiKeyAuth(TEST_API_KEY))


@pytest.mark.api
class TestMCPToolRegistration:
    """Verify all expected tools are registered on the MCP server."""

    async def test_all_tools_registered(self, _seed_mcp_services):
        async with Client(mcp) as client:
            tools = await client.list_tools()
        tool_names = {t.name for t in tools}

        expected_tools = {
            # Health
            "health_check",
            "readiness_check",
            # Dispatch
            "dispatch_create",
            "dispatch_get_status",
            "dispatch_cancel",
            # VM
            "vm_list",
            "vm_provision",
            "vm_provision_status",
            "vm_release",
            "vm_dry_run",
            # Run
            "run_provision",
            "run_execute",
            "run_teardown",
            "run_full",
            "run_status",
            # Config
            "config_get",
            # Events
            "events_query",
        }
        assert expected_tools.issubset(tool_names), f"Missing tools: {expected_tools - tool_names}"

    async def test_tool_descriptions_present(self, _seed_mcp_services):
        async with Client(mcp) as client:
            tools = await client.list_tools()
        for tool in tools:
            assert tool.description, f"Tool {tool.name} has no description"


@pytest.mark.api
class TestMCPAuth:
    """Test MCP API key authentication middleware."""

    async def test_health_no_auth_required(self, _seed_mcp_services, _mcp_auth):
        """Health tools should work even with auth middleware active."""
        async with Client(mcp) as client:
            result = await client.call_tool("health_check", {})
        assert "ok" in _text(result)

    async def test_readiness_no_auth_required(self, _seed_mcp_services, _mcp_auth):
        """Readiness tools should work even with auth middleware active."""
        async with Client(mcp) as client:
            result = await client.call_tool("readiness_check", {})
        assert "ready" in _text(result)

    async def test_non_health_tool_rejected_without_auth(self, _seed_mcp_services, _mcp_auth):
        """Non-health tools should fail without API key (stdio has no headers)."""
        async with Client(mcp) as client:
            with pytest.raises(ToolError, match="Invalid or missing API key"):
                await client.call_tool("config_get", {})


@pytest.mark.api
class TestMCPHealthTools:
    """Test health tool execution (no auth middleware)."""

    async def test_health_check_returns_status(self, _seed_mcp_services):
        async with Client(mcp) as client:
            result = await client.call_tool("health_check", {})
        data = json.loads(_text(result))
        assert data["status"] == "ok"
        assert data["version"] == "0.1.0"
        assert "uptime_seconds" in data

    async def test_readiness_check_returns_ready(self, _seed_mcp_services):
        async with Client(mcp) as client:
            result = await client.call_tool("readiness_check", {})
        data = json.loads(_text(result))
        assert data["status"] == "ready"


@pytest.mark.api
class TestMCPDispatchTools:
    """Test dispatch tool execution (no auth middleware)."""

    async def test_dispatch_create(self, _seed_mcp_services):
        async with Client(mcp) as client:
            result = await client.call_tool(
                "dispatch_create",
                {
                    "project": "test-project",
                    "phase": "do-task",
                    "branch": "main",
                    "spec_folder": "specs/test",
                    "cli": "claude",
                },
            )
        data = json.loads(_text(result))
        assert "dispatch_id" in data
        assert data["status"] == "accepted"

    async def test_dispatch_get_status(self, _seed_mcp_services):
        async with Client(mcp) as client:
            # Create first
            create_result = await client.call_tool(
                "dispatch_create",
                {
                    "project": "test-project",
                    "phase": "do-task",
                    "branch": "main",
                    "spec_folder": "specs/test",
                    "cli": "claude",
                },
            )
            created = json.loads(_text(create_result))
            dispatch_id = created["dispatch_id"]

            # Get status
            result = await client.call_tool(
                "dispatch_get_status",
                {"dispatch_id": dispatch_id},
            )
        data = json.loads(_text(result))
        assert data["workflow_id"] == dispatch_id
        assert data["project"] == "test-project"

    async def test_dispatch_cancel(self, _seed_mcp_services, tmp_path):
        # Use a separate store with no execution env so dispatch stays PENDING
        roles_yml = tmp_path / "cancel_roles.yml"
        roles_yml.write_text(
            "agents:\n  default:\n    cli: claude\n    model: sonnet\n    auth: oauth\n"
        )
        ipc_dir = tmp_path / "cancel_ipc"
        ipc_dir.mkdir()
        config = Config(
            ipc_dir=str(ipc_dir),
            github_dir=str(tmp_path / "github"),
            data_dir=str(tmp_path / "data"),
            worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
            roles_config_path=str(roles_yml),
        )
        store = APIStateStore()
        set_services(
            health=HealthService(),
            dispatch=DispatchService(
                store=store, config=config, emitter=AsyncMock(), execution_env=None
            ),
            vm=VMService(store=store),
            run=RunService(store=store),
            config=None,
            events=EventsService(MagicMock(events_db=None)),
        )

        async with Client(mcp) as client:
            create_result = await client.call_tool(
                "dispatch_create",
                {
                    "project": "test-project",
                    "phase": "do-task",
                    "branch": "main",
                    "spec_folder": "specs/test",
                    "cli": "claude",
                },
            )
            created = json.loads(_text(create_result))
            dispatch_id = created["dispatch_id"]

            cancel_result = await client.call_tool(
                "dispatch_cancel",
                {"dispatch_id": dispatch_id},
            )
        data = json.loads(_text(cancel_result))
        assert data["status"] == "cancelled"


@pytest.mark.api
class TestMCPVMTools:
    """Test VM tool execution."""

    async def test_vm_list_empty(self, _seed_mcp_services):
        async with Client(mcp) as client:
            result = await client.call_tool("vm_list", {})
        # Empty list → FastMCP returns empty content
        assert result.content == [] or _text(result) == "[]"

    async def test_vm_dry_run(self, _seed_mcp_services):
        async with Client(mcp) as client:
            result = await client.call_tool(
                "vm_dry_run",
                {"project": "test", "branch": "main"},
            )
        data = json.loads(_text(result))
        assert "requirements" in data


@pytest.mark.api
class TestMCPConfigTools:
    """Test config tool execution."""

    async def test_config_get(self, _seed_mcp_services):
        async with Client(mcp) as client:
            result = await client.call_tool("config_get", {})
        data = json.loads(_text(result))
        assert "ipc_dir" in data
        assert "poll_interval" in data


@pytest.mark.api
class TestMCPEventsTools:
    """Test events tool execution."""

    async def test_events_query_empty(self, _seed_mcp_services):
        async with Client(mcp) as client:
            result = await client.call_tool("events_query", {})
        data = json.loads(_text(result))
        assert "events" in data
        assert data["total"] == 0
