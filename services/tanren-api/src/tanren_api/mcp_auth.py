"""MCP authentication middleware — scoped API key resolution for MCP tools."""

from __future__ import annotations

import logging

import mcp.types as mt
from fastmcp.server.dependencies import get_http_headers
from fastmcp.server.middleware import Middleware, MiddlewareContext
from fastmcp.server.middleware.middleware import CallNext
from fastmcp.tools.tool import ToolResult
from mcp import McpError
from mcp.types import ErrorData

from tanren_api.key_utils import hash_api_key
from tanren_api.scopes import has_scope
from tanren_core.store.auth_protocols import AuthStore

logger = logging.getLogger(__name__)

# Tools that bypass authentication (matching REST health endpoints).
_PUBLIC_TOOLS = frozenset({"health_check", "readiness_check"})

# Map MCP tool names to required scopes.
_TOOL_SCOPE_MAP: dict[str, str] = {
    "dispatch_create": "dispatch:create",
    "dispatch_get_status": "dispatch:read",
    "dispatch_cancel": "dispatch:cancel",
    "vm_list": "vm:read",
    "vm_provision": "vm:provision",
    "vm_provision_status": "vm:read",
    "vm_release": "vm:release",
    "vm_dry_run": "vm:provision",
    "run_provision": "run:provision",
    "run_execute": "run:execute",
    "run_teardown": "run:teardown",
    "run_full": "run:full",
    "run_status": "run:read",
    "config_get": "config:read",
    "events_query": "events:read",
    "metrics_summary": "metrics:read",
    "metrics_costs": "metrics:read",
    "metrics_vms": "metrics:read",
}


class MCPApiKeyAuth(Middleware):
    """FastMCP middleware that validates and resolves X-API-Key on tool calls.

    Health tools are exempt from auth, matching the REST API behaviour.
    """

    def __init__(self, auth_store: AuthStore) -> None:
        """Initialize with the auth store for key resolution."""
        self._auth_store = auth_store

    async def on_call_tool(
        self,
        context: MiddlewareContext[mt.CallToolRequestParams],
        call_next: CallNext[mt.CallToolRequestParams, ToolResult],
    ) -> ToolResult:
        """Resolve API key and check scope before executing a tool call.

        Returns:
            ToolResult from the downstream tool handler.

        Raises:
            McpError: If the API key is invalid, revoked, or lacks required scope.
        """
        tool_name: str = context.message.name
        if tool_name in _PUBLIC_TOOLS:
            return await call_next(context)

        headers = get_http_headers() or {}
        api_key = headers.get("x-api-key", "")
        if not api_key:
            raise McpError(error=ErrorData(code=-32001, message="Missing API key"))

        key_hash = hash_api_key(api_key)
        key_view = await self._auth_store.get_api_key_by_hash(key_hash)
        if key_view is None:
            raise McpError(error=ErrorData(code=-32001, message="Invalid API key"))

        if key_view.revoked_at is not None:
            from datetime import UTC, datetime

            now = datetime.now(UTC).isoformat().replace("+00:00", "Z")
            if key_view.revoked_at <= now:
                raise McpError(error=ErrorData(code=-32001, message="API key has been revoked"))

        if key_view.expires_at is not None:
            from datetime import UTC, datetime

            now = datetime.now(UTC).isoformat().replace("+00:00", "Z")
            if key_view.expires_at <= now:
                raise McpError(error=ErrorData(code=-32001, message="API key has expired"))

        user = await self._auth_store.get_user(key_view.user_id)
        if user is None or not user.is_active:
            raise McpError(error=ErrorData(code=-32001, message="User not found or deactivated"))

        # Check tool-specific scope
        required_scope = _TOOL_SCOPE_MAP.get(tool_name)
        if required_scope:
            scopes = frozenset(key_view.scopes)
            if not has_scope(scopes, required_scope):
                raise McpError(
                    error=ErrorData(
                        code=-32003,
                        message=f"Missing required scope: {required_scope}",
                    )
                )

        return await call_next(context)
