"""Health check endpoints — no auth required."""

import time

from fastapi import APIRouter, Request

from tanren_api.models import HealthResponse

router = APIRouter(tags=["health"])

_start_time = time.monotonic()


@router.get("/api/v1/health")
async def health(request: Request) -> HealthResponse:
    """Return service health and version info."""
    return HealthResponse(
        status="ok",
        version="0.1.0",
        uptime_seconds=round(time.monotonic() - _start_time, 2),
    )


@router.get("/api/v1/health/ready")
async def readiness(request: Request) -> dict[str, str]:
    """Readiness probe.

    Returns:
        Dict with status key indicating readiness.
    """
    return {"status": "ready"}
