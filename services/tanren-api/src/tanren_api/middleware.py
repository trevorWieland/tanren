"""Request middleware for the tanren API."""

import logging
import time
import uuid
from collections.abc import MutableMapping
from typing import Any

from starlette.types import ASGIApp, Receive, Scope, Send

logger = logging.getLogger(__name__)

Message = MutableMapping[str, Any]


def RequestIDMiddleware(app: ASGIApp, /) -> ASGIApp:
    """Generate a UUID per request and attach it to the response.

    Returns:
        ASGI middleware that adds X-Request-ID headers.
    """

    async def middleware(scope: Scope, receive: Receive, send: Send) -> None:
        if scope["type"] != "http":
            await app(scope, receive, send)
            return

        request_id = str(uuid.uuid4())
        scope.setdefault("state", {})["request_id"] = request_id

        async def send_with_request_id(message: Message) -> None:
            if message.get("type") == "http.response.start":
                headers: list[Any] = list(message.get("headers", []))
                headers.append((b"x-request-id", request_id.encode()))
                message["headers"] = headers
            await send(message)

        await app(scope, receive, send_with_request_id)

    return middleware


def RequestLoggingMiddleware(app: ASGIApp, /) -> ASGIApp:
    """Log method, path, status code, and duration for every request.

    Returns:
        ASGI middleware that logs request details.
    """

    async def middleware(scope: Scope, receive: Receive, send: Send) -> None:
        if scope["type"] != "http":
            await app(scope, receive, send)
            return

        start = time.monotonic()
        status_code = 0

        async def send_with_logging(message: Message) -> None:
            nonlocal status_code
            if message.get("type") == "http.response.start":
                status_code = message.get("status", 0)
            await send(message)

        await app(scope, receive, send_with_logging)
        duration_ms = (time.monotonic() - start) * 1000
        logger.info(
            "%s %s %d %.1fms",
            scope.get("method", ""),
            scope.get("path", ""),
            status_code,
            duration_ms,
        )

    return middleware
