"""Metrics service — aggregated dashboard data from EventStore."""

from __future__ import annotations

from typing import TYPE_CHECKING

from tanren_api.models import (
    CostBucketResponse,
    MetricsCostsResponse,
    MetricsSummaryResponse,
    MetricsVMsResponse,
)

if TYPE_CHECKING:
    from tanren_core.store.protocols import EventStore


class MetricsService:
    """Service for querying aggregated dashboard metrics from EventStore."""

    def __init__(self, event_store: EventStore) -> None:
        """Initialize with the unified event store."""
        self._event_store = event_store

    async def summary(
        self,
        *,
        since: str | None = None,
        until: str | None = None,
        project: str | None = None,
    ) -> MetricsSummaryResponse:
        """Return workflow execution summary metrics from lifecycle events."""
        result = await self._event_store.query_events(
            event_type="PhaseCompleted",
            since=since,
            until=until,
            limit=10000,
        )

        total = 0
        succeeded = 0
        failed = 0
        errored = 0
        timed_out = 0
        blocked = 0
        durations: list[float] = []

        for row in result.events:
            payload = row.payload
            if project and str(payload.get("project", "")) != project:
                continue
            total += 1
            outcome = str(payload.get("outcome", ""))
            if outcome == "success":
                succeeded += 1
            elif outcome == "fail":
                failed += 1
            elif outcome == "error":
                errored += 1
            elif outcome == "timeout":
                timed_out += 1
            elif outcome == "blocked":
                blocked += 1
            dur = payload.get("duration_secs")
            if dur is not None:
                durations.append(float(str(dur)))

        durations.sort()
        avg = sum(durations) / len(durations) if durations else 0.0
        p50 = durations[len(durations) // 2] if durations else 0.0
        p95_idx = int(len(durations) * 0.95)
        p95 = durations[min(p95_idx, len(durations) - 1)] if durations else 0.0
        rate = succeeded / total if total > 0 else 0.0

        return MetricsSummaryResponse(
            total_phases=total,
            succeeded=succeeded,
            failed=failed,
            errored=errored,
            timed_out=timed_out,
            blocked=blocked,
            success_rate=round(rate, 4),
            avg_duration_secs=round(avg, 2),
            p50_duration_secs=round(p50, 2),
            p95_duration_secs=round(p95, 2),
        )

    async def costs(
        self,
        *,
        since: str | None = None,
        until: str | None = None,
        project: str | None = None,
        group_by: str = "model",
    ) -> MetricsCostsResponse:
        """Return token cost metrics from TokenUsageRecorded events."""
        result = await self._event_store.query_events(
            event_type="TokenUsageRecorded",
            since=since,
            until=until,
            limit=10000,
        )

        buckets_map: dict[str, CostBucketResponse] = {}
        total_cost = 0.0
        total_tokens = 0

        for row in result.events:
            payload = row.payload
            if project and str(payload.get("project", "")) != project:
                continue

            if group_by == "model":
                models_used = payload.get("models_used", [])
                if isinstance(models_used, list) and models_used:
                    key = str(models_used[0])
                else:
                    key = str(payload.get("model", "unknown"))
            elif group_by == "day":
                key = str(payload.get("timestamp", ""))[:10]
            else:
                key = str(payload.get("workflow_id", "unknown"))

            cost = float(str(payload.get("total_cost", 0)))
            tokens = int(str(payload.get("total_tokens", 0)))
            total_cost += cost
            total_tokens += tokens

            if key not in buckets_map:
                buckets_map[key] = CostBucketResponse(
                    group_key=key,
                    total_cost=0.0,
                    total_tokens=0,
                    input_tokens=0,
                    output_tokens=0,
                    event_count=0,
                )
            b = buckets_map[key]
            buckets_map[key] = CostBucketResponse(
                group_key=key,
                total_cost=b.total_cost + cost,
                total_tokens=b.total_tokens + tokens,
                input_tokens=b.input_tokens + int(str(payload.get("input_tokens", 0))),
                output_tokens=b.output_tokens + int(str(payload.get("output_tokens", 0))),
                cache_read_tokens=b.cache_read_tokens
                + int(str(payload.get("cache_read_tokens", 0))),
                cache_creation_tokens=b.cache_creation_tokens
                + int(str(payload.get("cache_creation_tokens", 0))),
                reasoning_tokens=b.reasoning_tokens + int(str(payload.get("reasoning_tokens", 0))),
                event_count=b.event_count + 1,
            )

        return MetricsCostsResponse(
            buckets=list(buckets_map.values()),
            total_cost=total_cost,
            total_tokens=total_tokens,
            group_by=group_by,
        )

    async def vms(
        self,
        *,
        since: str | None = None,
        until: str | None = None,
        project: str | None = None,
    ) -> MetricsVMsResponse:
        """Return VM utilization metrics from lifecycle events."""
        prov_result = await self._event_store.query_events(
            event_type="VMProvisioned",
            since=since,
            until=until,
            limit=10000,
        )
        rel_result = await self._event_store.query_events(
            event_type="VMReleased",
            since=since,
            until=until,
            limit=10000,
        )

        provisioned = 0
        released = 0
        total_duration = 0
        total_cost = 0.0
        by_provider: dict[str, int] = {}

        for row in prov_result.events:
            p = row.payload
            if project and str(p.get("project", "")) != project:
                continue
            provisioned += 1
            provider = str(p.get("provider", "unknown"))
            by_provider[provider] = by_provider.get(provider, 0) + 1

        for row in rel_result.events:
            p = row.payload
            if project and str(p.get("project", "")) != project:
                continue
            released += 1
            dur = p.get("duration_secs", 0)
            total_duration += int(str(dur)) if dur else 0
            cost = p.get("estimated_cost")
            if cost is not None:
                total_cost += float(str(cost))

        avg_dur = total_duration / released if released > 0 else 0.0

        return MetricsVMsResponse(
            total_provisioned=provisioned,
            total_released=released,
            currently_active=max(0, provisioned - released),
            total_vm_duration_secs=total_duration,
            total_estimated_cost=total_cost,
            avg_duration_secs=round(avg_dur, 2),
            by_provider=by_provider,
        )
