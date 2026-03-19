"""Metrics service — aggregated dashboard data."""

from __future__ import annotations

from typing import TYPE_CHECKING

from tanren_api.models import (
    CostBucketResponse,
    MetricsCostsResponse,
    MetricsSummaryResponse,
    MetricsVMsResponse,
)

if TYPE_CHECKING:
    from tanren_core.adapters.metrics_reader import MetricsReader


class MetricsService:
    """Service for querying aggregated dashboard metrics."""

    def __init__(self, metrics_reader: MetricsReader | None = None) -> None:
        """Initialize with an optional metrics reader backend."""
        self._reader = metrics_reader

    async def summary(
        self,
        *,
        since: str | None = None,
        until: str | None = None,
        project: str | None = None,
    ) -> MetricsSummaryResponse:
        """Return workflow execution summary metrics."""
        if self._reader is None:
            return MetricsSummaryResponse(
                total_phases=0,
                succeeded=0,
                failed=0,
                errored=0,
                timed_out=0,
                blocked=0,
                success_rate=0.0,
                avg_duration_secs=0.0,
                p50_duration_secs=0.0,
                p95_duration_secs=0.0,
            )
        result = await self._reader.query_summary(since=since, until=until, project=project)
        rate = result.succeeded / result.total_phases if result.total_phases > 0 else 0.0
        return MetricsSummaryResponse(
            total_phases=result.total_phases,
            succeeded=result.succeeded,
            failed=result.failed,
            errored=result.errored,
            timed_out=result.timed_out,
            blocked=result.blocked,
            success_rate=round(rate, 4),
            avg_duration_secs=result.avg_duration_secs,
            p50_duration_secs=result.p50_duration_secs,
            p95_duration_secs=result.p95_duration_secs,
        )

    async def costs(
        self,
        *,
        since: str | None = None,
        until: str | None = None,
        project: str | None = None,
        group_by: str = "model",
    ) -> MetricsCostsResponse:
        """Return token cost metrics grouped by model, day, or workflow."""
        if self._reader is None:
            return MetricsCostsResponse(
                buckets=[], total_cost=0.0, total_tokens=0, group_by=group_by
            )
        result = await self._reader.query_costs(
            since=since, until=until, project=project, group_by=group_by
        )
        buckets = [
            CostBucketResponse(
                group_key=b.group_key,
                total_cost=b.total_cost,
                total_tokens=b.total_tokens,
                input_tokens=b.input_tokens,
                output_tokens=b.output_tokens,
                cache_read_tokens=b.cache_read_tokens,
                cache_creation_tokens=b.cache_creation_tokens,
                reasoning_tokens=b.reasoning_tokens,
                event_count=b.event_count,
            )
            for b in result.buckets
        ]
        return MetricsCostsResponse(
            buckets=buckets,
            total_cost=result.total_cost,
            total_tokens=result.total_tokens,
            group_by=group_by,
        )

    async def vms(
        self,
        *,
        since: str | None = None,
        until: str | None = None,
        project: str | None = None,
    ) -> MetricsVMsResponse:
        """Return VM utilization metrics."""
        if self._reader is None:
            return MetricsVMsResponse(
                total_provisioned=0,
                total_released=0,
                currently_active=0,
                total_vm_duration_secs=0,
                total_estimated_cost=0.0,
                avg_duration_secs=0.0,
            )
        result = await self._reader.query_vms(since=since, until=until, project=project)
        return MetricsVMsResponse(
            total_provisioned=result.total_provisioned,
            total_released=result.total_released,
            currently_active=result.currently_active,
            total_vm_duration_secs=result.total_vm_duration_secs,
            total_estimated_cost=result.total_estimated_cost,
            avg_duration_secs=result.avg_duration_secs,
            by_provider=result.by_provider,
        )
