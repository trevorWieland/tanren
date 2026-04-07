"""API error types and global exception handler."""

from fastapi import Request
from fastapi.responses import JSONResponse

from tanren_api.models import ErrorResponse
from tanren_core.timestamps import utc_now_iso


class TanrenAPIError(Exception):
    """Base exception with HTTP status and error code."""

    def __init__(self, status_code: int, error_code: str, detail: str) -> None:
        """Initialize with HTTP status code, error code, and detail message."""
        self.status_code = status_code
        self.error_code = error_code
        self.detail = detail
        super().__init__(detail)


class NotFoundError(TanrenAPIError):
    """Resource not found (404)."""

    def __init__(self, detail: str = "Resource not found") -> None:
        """Initialize with optional detail message."""
        super().__init__(404, "not_found", detail)


class AuthenticationError(TanrenAPIError):
    """Authentication failed (401)."""

    def __init__(self, detail: str = "Authentication failed") -> None:
        """Initialize with optional detail message."""
        super().__init__(401, "authentication_error", detail)


class NotImplementedAPIError(TanrenAPIError):
    """Endpoint not yet implemented (501)."""

    def __init__(self, detail: str = "Not implemented") -> None:
        """Initialize with optional detail message."""
        super().__init__(501, "not_implemented", detail)


class ValidationError(TanrenAPIError):
    """Client input validation error (400)."""

    def __init__(self, detail: str = "Validation error") -> None:
        """Initialize with optional detail message."""
        super().__init__(400, "validation_error", detail)


class ForbiddenError(TanrenAPIError):
    """Insufficient permissions (403)."""

    def __init__(self, detail: str = "Forbidden") -> None:
        """Initialize with optional detail message."""
        super().__init__(403, "forbidden", detail)


class ConflictError(TanrenAPIError):
    """Conflict (409)."""

    def __init__(self, detail: str = "Conflict") -> None:
        """Initialize with optional detail message."""
        super().__init__(409, "conflict", detail)


class ServiceError(TanrenAPIError):
    """Internal server error (500)."""

    def __init__(self, detail: str = "Internal server error") -> None:
        """Initialize with optional detail message."""
        super().__init__(500, "service_error", detail)


async def tanren_error_handler(request: Request, exc: Exception) -> JSONResponse:  # noqa: RUF029 — FastAPI requires async exception handlers
    """Global exception handler returning consistent ErrorResponse bodies.

    Returns:
        JSONResponse with structured error body.
    """
    assert isinstance(exc, TanrenAPIError)
    request_id = getattr(request.state, "request_id", None)
    body = ErrorResponse(
        detail=exc.detail,
        error_code=exc.error_code,
        timestamp=utc_now_iso(),
        request_id=request_id,
    )
    return JSONResponse(status_code=exc.status_code, content=body.model_dump())
