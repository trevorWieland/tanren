"""MCP authentication middleware — reuses APIKeyVerifier for API key validation."""

from __future__ import annotations

import logging

import mcp.types as mt
from fastmcp.server.dependencies import get_http_headers
from fastmcp.server.middleware import Middleware, MiddlewareContext
from fastmcp.server.middleware.middleware import CallNext
from fastmcp.tools.tool import ToolResult

from tanren_api.auth import APIKeyVerifier
from tanren_api.errors import AuthenticationError

logger = logging.getLogger(__name__)

# Tools that bypass authentication (matching REST health endpoints).
_PUBLIC_TOOLS = frozenset({"health_check", "readiness_check"})


class MCPApiKeyAuth(Middleware):
    """FastMCP middleware that validates X-API-Key header on tool calls.

    Health tools are exempt from auth, matching the REST API behaviour.
    """

    def __init__(self, api_key: str) -> None:
        """Initialize with the expected API key."""
        self._verifier = APIKeyVerifier(api_key)

    async def on_call_tool(
        self,
        context: MiddlewareContext[mt.CallToolRequestParams],
        call_next: CallNext[mt.CallToolRequestParams, ToolResult],
    ) -> ToolResult:
        """Verify API key before executing a tool call.

        Returns:
            ToolResult from the downstream tool handler.

        Raises:
            McpError: If the API key is invalid or missing.
        """
        tool_name: str = context.message.name
        if tool_name in _PUBLIC_TOOLS:
            return await call_next(context)

        headers = get_http_headers() or {}
        api_key = headers.get("x-api-key", "")
        try:
            await self._verifier.verify(api_key)
        except AuthenticationError:
            from mcp import McpError  # noqa: PLC0415 — deferred import for exception handling
            from mcp.types import ErrorData  # noqa: PLC0415 — deferred import for exception handling

            raise McpError(
                error=ErrorData(code=-32001, message="Invalid or missing API key")
            ) from None

        return await call_next(context)
