"""Metrics endpoints — aggregated dashboard data."""
# ruff: noqa: DOC201 — FastAPI endpoints return Response objects implicitly

from __future__ import annotations

from typing import Annotated

from fastapi import APIRouter, Depends, Query

from tanren_api.dependencies import get_metrics_reader
from tanren_api.models import (
    CostGroupBy,
    MetricsCostsResponse,
    MetricsSummaryResponse,
    MetricsVMsResponse,
)
from tanren_api.services.metrics import MetricsService
from tanren_core.adapters.metrics_reader import MetricsReader

router = APIRouter(tags=["metrics"])


@router.get("/metrics/summary")
async def metrics_summary(
    metrics_reader: Annotated[MetricsReader | None, Depends(get_metrics_reader)],
    since: Annotated[str | None, Query(description="ISO 8601 start (inclusive)")] = None,
    until: Annotated[str | None, Query(description="ISO 8601 end (inclusive)")] = None,
    project: Annotated[str | None, Query(description="Filter by project")] = None,
) -> MetricsSummaryResponse:
    """Workflow success/failure rate and duration stats."""
    return await MetricsService(metrics_reader).summary(since=since, until=until, project=project)


@router.get("/metrics/costs")
async def metrics_costs(
    metrics_reader: Annotated[MetricsReader | None, Depends(get_metrics_reader)],
    since: Annotated[str | None, Query(description="ISO 8601 start (inclusive)")] = None,
    until: Annotated[str | None, Query(description="ISO 8601 end (inclusive)")] = None,
    project: Annotated[str | None, Query(description="Filter by project")] = None,
    group_by: Annotated[CostGroupBy, Query(description="Aggregation grouping")] = CostGroupBy.MODEL,
) -> MetricsCostsResponse:
    """Token cost metrics grouped by model, day, or workflow."""
    return await MetricsService(metrics_reader).costs(
        since=since, until=until, project=project, group_by=group_by.value
    )


@router.get("/metrics/vms")
async def metrics_vms(
    metrics_reader: Annotated[MetricsReader | None, Depends(get_metrics_reader)],
    since: Annotated[str | None, Query(description="ISO 8601 start (inclusive)")] = None,
    until: Annotated[str | None, Query(description="ISO 8601 end (inclusive)")] = None,
    project: Annotated[str | None, Query(description="Filter by project")] = None,
) -> MetricsVMsResponse:
    """VM utilization metrics."""
    return await MetricsService(metrics_reader).vms(since=since, until=until, project=project)
