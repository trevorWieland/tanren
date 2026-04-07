"""Metrics endpoints — aggregated dashboard data."""

from __future__ import annotations

from typing import Annotated

from fastapi import APIRouter, Depends, Query

from tanren_api.auth import require_scope
from tanren_api.dependencies import get_event_store
from tanren_api.models import (
    CostGroupBy,
    MetricsCostsResponse,
    MetricsSummaryResponse,
    MetricsVMsResponse,
)
from tanren_api.services.metrics import MetricsService
from tanren_core.store.auth_views import AuthContext
from tanren_core.store.protocols import EventStore

router = APIRouter(tags=["metrics"])


@router.get("/metrics/summary")
async def metrics_summary(
    _auth: Annotated[AuthContext, Depends(require_scope("metrics:read"))],
    event_store: Annotated[EventStore, Depends(get_event_store)],
    since: Annotated[str | None, Query(description="ISO 8601 start (inclusive)")] = None,
    until: Annotated[str | None, Query(description="ISO 8601 end (inclusive)")] = None,
    project: Annotated[str | None, Query(description="Filter by project")] = None,
) -> MetricsSummaryResponse:
    """Workflow success/failure rate and duration stats."""
    return await MetricsService(event_store).summary(since=since, until=until, project=project)


@router.get("/metrics/costs")
async def metrics_costs(
    _auth: Annotated[AuthContext, Depends(require_scope("metrics:read"))],
    event_store: Annotated[EventStore, Depends(get_event_store)],
    since: Annotated[str | None, Query(description="ISO 8601 start (inclusive)")] = None,
    until: Annotated[str | None, Query(description="ISO 8601 end (inclusive)")] = None,
    project: Annotated[str | None, Query(description="Filter by project")] = None,
    group_by: Annotated[CostGroupBy, Query(description="Aggregation grouping")] = CostGroupBy.MODEL,
) -> MetricsCostsResponse:
    """Token cost metrics grouped by model, day, or workflow."""
    return await MetricsService(event_store).costs(
        since=since, until=until, project=project, group_by=group_by.value
    )


@router.get("/metrics/vms")
async def metrics_vms(
    _auth: Annotated[AuthContext, Depends(require_scope("metrics:read"))],
    event_store: Annotated[EventStore, Depends(get_event_store)],
    since: Annotated[str | None, Query(description="ISO 8601 start (inclusive)")] = None,
    until: Annotated[str | None, Query(description="ISO 8601 end (inclusive)")] = None,
    project: Annotated[str | None, Query(description="Filter by project")] = None,
) -> MetricsVMsResponse:
    """VM utilization metrics."""
    return await MetricsService(event_store).vms(since=since, until=until, project=project)
