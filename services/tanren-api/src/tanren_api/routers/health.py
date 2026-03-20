"""Health check endpoints — no auth required."""

from fastapi import APIRouter

from tanren_api.models import HealthResponse, ReadinessResponse
from tanren_api.services import HealthService

router = APIRouter(tags=["health"])

_svc = HealthService()


@router.get("/api/v1/health")
async def health() -> HealthResponse:
    """Return service health and version info.

    Returns:
        HealthResponse: Service health status and version.
    """
    return await _svc.health()


@router.get("/api/v1/health/ready")
async def readiness() -> ReadinessResponse:
    """Readiness probe.

    Returns:
        ReadinessResponse: Service readiness status.
    """
    return await _svc.readiness()
