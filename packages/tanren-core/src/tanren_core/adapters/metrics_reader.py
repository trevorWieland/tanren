"""Metrics reader protocol and result types for dashboard aggregation."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Protocol, runtime_checkable

# ---------------------------------------------------------------------------
# Result dataclasses
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class SummaryMetrics:
    """Aggregated workflow summary metrics."""

    total_phases: int = 0
    succeeded: int = 0
    failed: int = 0
    errored: int = 0
    timed_out: int = 0
    blocked: int = 0
    avg_duration_secs: float = 0.0
    p50_duration_secs: float = 0.0
    p95_duration_secs: float = 0.0


@dataclass(frozen=True)
class CostBucket:
    """Single aggregation bucket for cost metrics."""

    group_key: str
    total_cost: float = 0.0
    total_tokens: int = 0
    input_tokens: int = 0
    output_tokens: int = 0
    cache_read_tokens: int = 0
    cache_creation_tokens: int = 0
    reasoning_tokens: int = 0
    event_count: int = 0


@dataclass(frozen=True)
class CostMetrics:
    """Aggregated cost metrics."""

    buckets: list[CostBucket] = field(default_factory=list)
    total_cost: float = 0.0
    total_tokens: int = 0


@dataclass(frozen=True)
class VMMetrics:
    """Aggregated VM utilization metrics."""

    total_provisioned: int = 0
    total_released: int = 0
    currently_active: int = 0
    total_vm_duration_secs: int = 0
    total_estimated_cost: float = 0.0
    avg_duration_secs: float = 0.0
    by_provider: dict[str, int] = field(default_factory=dict)


# ---------------------------------------------------------------------------
# Protocol
# ---------------------------------------------------------------------------


@runtime_checkable
class MetricsReader(Protocol):
    """Protocol for reading aggregated metrics from events."""

    async def query_summary(
        self,
        *,
        since: str | None = None,
        until: str | None = None,
        project: str | None = None,
    ) -> SummaryMetrics:
        """Query workflow execution summary metrics."""
        ...

    async def query_costs(
        self,
        *,
        since: str | None = None,
        until: str | None = None,
        project: str | None = None,
        group_by: str = "model",
    ) -> CostMetrics:
        """Query token cost metrics grouped by model, day, or workflow."""
        ...

    async def query_vms(
        self,
        *,
        since: str | None = None,
        until: str | None = None,
        project: str | None = None,
    ) -> VMMetrics:
        """Query VM utilization metrics."""
        ...
