"""Health check endpoints — no auth required."""
# ruff: noqa: DOC201

from fastapi import APIRouter

from tanren_api.models import HealthResponse, ReadinessResponse
from tanren_api.services import HealthService

router = APIRouter(tags=["health"])

_svc = HealthService()


@router.get("/api/v1/health")
async def health() -> HealthResponse:
    """Return service health and version info."""
    return await _svc.health()


@router.get("/api/v1/health/ready")
async def readiness() -> ReadinessResponse:
    """Readiness probe."""
    return await _svc.readiness()
