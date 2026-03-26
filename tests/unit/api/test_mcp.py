"""Tests for MCP server — tool registration, auth middleware, and tool execution."""

from __future__ import annotations

import json
from typing import TYPE_CHECKING

import pytest
from fastmcp import Client
from fastmcp.exceptions import ToolError

from tanren_api.mcp_auth import MCPApiKeyAuth
from tanren_api.mcp_server import mcp, set_services, set_worker_config
from tanren_api.services import (
    ConfigService,
    DispatchService,
    EventsService,
    HealthService,
    MetricsService,
    RunService,
    VMService,
)
from tanren_api.settings import APISettings
from tanren_core.store.sqlite import SqliteStore

if TYPE_CHECKING:
    from pathlib import Path

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
async def mcp_store(tmp_path: Path):
    store = SqliteStore(tmp_path / "mcp-test.db")
    await store._ensure_conn()
    yield store
    await store.close()


@pytest.fixture
def _seed_mcp_services(mcp_store, tmp_path: Path):
    """Seed MCP service singletons with store-based dependencies."""
    # Create a minimal project layout for dispatch resolution
    from tanren_core.worker_config import WorkerConfig

    github_dir = tmp_path / "github"
    project_dir = github_dir / "test-project"
    project_dir.mkdir(parents=True)
    (project_dir / "tanren.yml").write_text("environment:\n  default:\n    type: local\n")
    config = WorkerConfig(
        ipc_dir=str(tmp_path / "ipc"),
        github_dir=str(github_dir),
        data_dir=str(tmp_path / "data"),
        db_url=str(tmp_path / "events.db"),
        worktree_registry_path=str(tmp_path / "worktrees.json"),
    )
    set_worker_config(config)

    settings = APISettings(api_key=TEST_API_KEY)

    set_services(
        health=HealthService(),
        dispatch=DispatchService(
            event_store=mcp_store,
            job_queue=mcp_store,
            state_store=mcp_store,
        ),
        vm=VMService(
            event_store=mcp_store,
            job_queue=mcp_store,
            state_store=mcp_store,
        ),
        run=RunService(
            event_store=mcp_store,
            job_queue=mcp_store,
            state_store=mcp_store,
        ),
        config=ConfigService(settings, mcp_store),
        events=EventsService(mcp_store),
        metrics=MetricsService(mcp_store),
    )


@pytest.fixture
async def _mcp_auth(mcp_store):
    """Add auth middleware with seeded store to the MCP server for this test scope."""
    from tanren_api.auth_seed import seed_legacy_admin_key

    await seed_legacy_admin_key(mcp_store, mcp_store, TEST_API_KEY)
    mcp.add_middleware(MCPApiKeyAuth(mcp_store))


@pytest.mark.api
class TestMCPToolRegistration:
    """Verify all expected tools are registered on the MCP server."""

    @pytest.mark.usefixtures("_seed_mcp_services")
    async def test_all_tools_registered(self):
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

    @pytest.mark.usefixtures("_seed_mcp_services")
    async def test_tool_descriptions_present(self):
        async with Client(mcp) as client:
            tools = await client.list_tools()
        for tool in tools:
            assert tool.description, f"Tool {tool.name} has no description"


@pytest.mark.api
class TestMCPAuth:
    """Test MCP API key authentication middleware."""

    @pytest.mark.usefixtures("_seed_mcp_services", "_mcp_auth")
    async def test_health_no_auth_required(self):
        """Health tools should work even with auth middleware active."""
        async with Client(mcp) as client:
            result = await client.call_tool("health_check", {})
        assert "ok" in _text(result)

    @pytest.mark.usefixtures("_seed_mcp_services", "_mcp_auth")
    async def test_readiness_no_auth_required(self):
        """Readiness tools should work even with auth middleware active."""
        async with Client(mcp) as client:
            result = await client.call_tool("readiness_check", {})
        assert "ready" in _text(result)

    @pytest.mark.usefixtures("_seed_mcp_services", "_mcp_auth")
    async def test_non_health_tool_rejected_without_auth(self):
        """Non-health tools should fail without API key (stdio has no headers)."""
        async with Client(mcp) as client:
            with pytest.raises(ToolError, match="Missing API key"):
                await client.call_tool("config_get", {})


@pytest.mark.api
class TestMCPHealthTools:
    """Test health tool execution (no auth middleware)."""

    @pytest.mark.usefixtures("_seed_mcp_services")
    async def test_health_check_returns_status(self):
        async with Client(mcp) as client:
            result = await client.call_tool("health_check", {})
        data = json.loads(_text(result))
        assert data["status"] == "ok"
        assert data["version"] == "0.1.0"
        assert "uptime_seconds" in data

    @pytest.mark.usefixtures("_seed_mcp_services")
    async def test_readiness_check_returns_ready(self):
        async with Client(mcp) as client:
            result = await client.call_tool("readiness_check", {})
        data = json.loads(_text(result))
        assert data["status"] == "ready"


@pytest.mark.api
class TestMCPDispatchTools:
    """Test dispatch tool execution (no auth middleware)."""

    @pytest.mark.usefixtures("_seed_mcp_services")
    async def test_dispatch_create(self):
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

    @pytest.mark.usefixtures("_seed_mcp_services")
    async def test_dispatch_get_status(self):
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


@pytest.mark.api
class TestMCPConfigTools:
    """Test config tool execution."""

    @pytest.mark.usefixtures("_seed_mcp_services")
    async def test_config_get(self):
        async with Client(mcp) as client:
            result = await client.call_tool("config_get", {})
        data = json.loads(_text(result))
        assert "db_backend" in data
        assert "store_connected" in data
        assert "version" in data


@pytest.mark.api
class TestMCPEventsTools:
    """Test events tool execution."""

    @pytest.mark.usefixtures("_seed_mcp_services")
    async def test_events_query_empty(self):
        async with Client(mcp) as client:
            result = await client.call_tool("events_query", {})
        data = json.loads(_text(result))
        assert "events" in data
        assert data["total"] == 0

    @pytest.mark.usefixtures("_seed_mcp_services")
    async def test_events_query_clamps_limit(self):
        """Negative or oversized limits are clamped to [1, 100]."""
        async with Client(mcp) as client:
            # Negative limit clamped to 1
            r1 = await client.call_tool("events_query", {"limit": -1})
            d1 = json.loads(_text(r1))
            assert d1["limit"] == 1

            # Oversized limit clamped to 100
            r2 = await client.call_tool("events_query", {"limit": 999})
            d2 = json.loads(_text(r2))
            assert d2["limit"] == 100

    @pytest.mark.usefixtures("_seed_mcp_services")
    async def test_events_query_clamps_offset(self):
        """Negative offset is clamped to 0."""
        async with Client(mcp) as client:
            result = await client.call_tool("events_query", {"offset": -5})
        data = json.loads(_text(result))
        assert data["offset"] == 0


@pytest.mark.api
class TestMCPMiddlewareStacking:
    """Test that auth middleware doesn't accumulate on repeated lifespan entries."""

    async def test_no_duplicate_auth_middleware(self):
        """Adding auth middleware twice should replace, not stack."""
        from unittest.mock import MagicMock

        from tanren_core.store.auth_protocols import AuthStore

        mock_store = MagicMock(spec=AuthStore)
        saved = list(mcp.middleware)
        try:
            mcp.middleware.clear()

            # Simulate two lifespan entries (same pattern as main.py)
            for _ in range(3):
                mcp.middleware[:] = [m for m in mcp.middleware if not isinstance(m, MCPApiKeyAuth)]
                mcp.add_middleware(MCPApiKeyAuth(mock_store))

            auth_count = sum(1 for m in mcp.middleware if isinstance(m, MCPApiKeyAuth))
            assert auth_count == 1, f"Expected 1 auth middleware, got {auth_count}"
        finally:
            mcp.middleware.clear()
            mcp.middleware.extend(saved)
