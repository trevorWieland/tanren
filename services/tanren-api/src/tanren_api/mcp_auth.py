"""MCP authentication middleware — reuses APIKeyVerifier for API key validation."""
# ruff: noqa: DOC201,DOC501,ANN401

from __future__ import annotations

import logging
from typing import Any

from fastmcp.server.dependencies import get_http_headers
from fastmcp.server.middleware import Middleware, MiddlewareContext

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

    async def on_call_tool(self, context: MiddlewareContext, call_next: Any) -> Any:
        """Verify API key before executing a tool call."""
        tool_name: str = context.message.name
        if tool_name in _PUBLIC_TOOLS:
            return await call_next(context)

        headers = get_http_headers() or {}
        api_key = headers.get("x-api-key", "")
        try:
            await self._verifier.verify(api_key)
        except AuthenticationError:
            from mcp import McpError  # noqa: PLC0415
            from mcp.types import ErrorData  # noqa: PLC0415

            raise McpError(
                error=ErrorData(code=-32001, message="Invalid or missing API key")
            ) from None

        return await call_next(context)
