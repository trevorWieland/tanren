"""Metrics service — aggregated dashboard data from EventStore."""

from __future__ import annotations

import math
import statistics
from typing import TYPE_CHECKING

from tanren_api.models import (
    CostBucketResponse,
    MetricsCostsResponse,
    MetricsSummaryResponse,
    MetricsVMsResponse,
)

if TYPE_CHECKING:
    from tanren_core.store.protocols import EventStore
    from tanren_core.store.views import EventRow

_PAGE_SIZE = 5000


def _safe_float(value: object, default: float = 0.0) -> float:
    """Parse a float from an event payload value, returning default on failure."""
    try:
        return float(str(value))
    except ValueError, TypeError:
        return default


def _safe_int(value: object, default: int = 0) -> int:
    """Parse an int from an event payload value, returning default on failure."""
    try:
        return int(str(value))
    except ValueError, TypeError:
        return default


class MetricsService:
    """Service for querying aggregated dashboard metrics from EventStore."""

    def __init__(self, event_store: EventStore) -> None:
        """Initialize with the unified event store."""
        self._event_store = event_store

    async def _fetch_all_events(
        self,
        *,
        event_type: str,
        since: str | None = None,
        until: str | None = None,
    ) -> list[EventRow]:
        """Paginate through all matching events.

        Returns:
            Complete list of EventRow objects across all pages.
        """
        all_events: list[EventRow] = []
        offset = 0
        while True:
            result = await self._event_store.query_events(
                event_type=event_type,
                since=since,
                until=until,
                limit=_PAGE_SIZE,
                offset=offset,
            )
            all_events.extend(result.events)
            if len(result.events) < _PAGE_SIZE:
                break
            offset += _PAGE_SIZE
        return all_events

    async def summary(
        self,
        *,
        since: str | None = None,
        until: str | None = None,
        project: str | None = None,
    ) -> MetricsSummaryResponse:
        """Return workflow execution summary metrics from lifecycle events."""
        events = await self._fetch_all_events(event_type="PhaseCompleted", since=since, until=until)

        counts: dict[str, int] = {
            "success": 0,
            "fail": 0,
            "error": 0,
            "timeout": 0,
            "blocked": 0,
        }
        total = 0
        durations: list[float] = []

        for row in events:
            payload = row.payload
            if project and str(payload.get("project", "")) != project:
                continue
            total += 1
            outcome = str(payload.get("outcome", ""))
            if outcome in counts:
                counts[outcome] += 1
            dur = payload.get("duration_secs")
            if dur is not None:
                dur_val = _safe_float(dur, default=-1.0)
                if dur_val >= 0:
                    durations.append(dur_val)

        durations.sort()
        avg = sum(durations) / len(durations) if durations else 0.0
        p50 = statistics.median(durations) if durations else 0.0
        p95_idx = min(math.ceil(len(durations) * 0.95) - 1, len(durations) - 1)
        p95 = durations[p95_idx] if durations else 0.0
        rate = counts["success"] / total if total > 0 else 0.0

        return MetricsSummaryResponse(
            total_phases=total,
            succeeded=counts["success"],
            failed=counts["fail"],
            errored=counts["error"],
            timed_out=counts["timeout"],
            blocked=counts["blocked"],
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
        events = await self._fetch_all_events(
            event_type="TokenUsageRecorded", since=since, until=until
        )

        buckets_map: dict[str, CostBucketResponse] = {}
        total_cost = 0.0
        total_tokens = 0

        for row in events:
            payload = row.payload
            if project and str(payload.get("project", "")) != project:
                continue

            if group_by == "model":
                models_used = payload.get("models_used", [])
                if isinstance(models_used, list) and models_used:
                    key = ", ".join(sorted(str(m) for m in models_used))
                else:
                    key = str(payload.get("model", "unknown"))
            elif group_by == "day":
                key = str(payload.get("timestamp", ""))[:10]
            else:
                key = str(payload.get("entity_id", payload.get("workflow_id", "unknown")))

            cost = _safe_float(payload.get("total_cost", 0))
            tokens = _safe_int(payload.get("total_tokens", 0))
            total_cost += cost
            total_tokens += tokens

            b = buckets_map.setdefault(
                key,
                CostBucketResponse(
                    group_key=key,
                    total_cost=0.0,
                    total_tokens=0,
                    input_tokens=0,
                    output_tokens=0,
                    event_count=0,
                ),
            )
            buckets_map[key] = CostBucketResponse(
                group_key=key,
                total_cost=b.total_cost + cost,
                total_tokens=b.total_tokens + tokens,
                input_tokens=b.input_tokens + _safe_int(payload.get("input_tokens", 0)),
                output_tokens=b.output_tokens + _safe_int(payload.get("output_tokens", 0)),
                cache_read_tokens=b.cache_read_tokens
                + _safe_int(payload.get("cache_read_tokens", 0)),
                cache_creation_tokens=b.cache_creation_tokens
                + _safe_int(payload.get("cache_creation_tokens", 0)),
                reasoning_tokens=b.reasoning_tokens + _safe_int(payload.get("reasoning_tokens", 0)),
                event_count=b.event_count + 1,
            )

        return MetricsCostsResponse(
            buckets=sorted(buckets_map.values(), key=lambda b: b.group_key),
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
        prov_events = await self._fetch_all_events(
            event_type="VMProvisioned", since=since, until=until
        )
        rel_events = await self._fetch_all_events(event_type="VMReleased", since=since, until=until)

        provisioned = 0
        released = 0
        total_duration = 0
        total_cost = 0.0
        by_provider: dict[str, int] = {}

        for row in prov_events:
            p = row.payload
            if project and str(p.get("project", "")) != project:
                continue
            provisioned += 1
            provider = str(p.get("provider", "unknown"))
            by_provider[provider] = by_provider.get(provider, 0) + 1

        for row in rel_events:
            p = row.payload
            if project and str(p.get("project", "")) != project:
                continue
            released += 1
            total_duration += _safe_int(p.get("duration_secs", 0))
            cost = p.get("estimated_cost")
            if cost is not None:
                total_cost += _safe_float(cost)

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
