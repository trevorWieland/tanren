# ruff: noqa: DOC201
"""Postgres-backed metrics reader for dashboard aggregation."""

from __future__ import annotations

import json
import logging
from collections import defaultdict
from typing import TYPE_CHECKING

from tanren_core.adapters.metrics_reader import (
    CostBucket,
    CostMetrics,
    SummaryMetrics,
    VMMetrics,
)

if TYPE_CHECKING:
    import asyncpg

logger = logging.getLogger(__name__)


def _canonical_model_key(models_used: list[str]) -> str:
    """Return a canonical JSON string key for a sorted model list."""
    return json.dumps(sorted(models_used))


class PostgresMetricsReader:
    """Reads aggregated metrics from a Postgres events database.

    Satisfies the MetricsReader protocol via structural typing.
    """

    def __init__(self, pool: asyncpg.Pool) -> None:
        """Initialize with an asyncpg connection pool."""
        self._pool = pool

    def _build_where(
        self,
        event_type: str,
        *,
        since: str | None,
        until: str | None,
        project: str | None,
    ) -> tuple[str, list[str | int], int]:
        """Build WHERE clause. Returns (sql, params, next_idx)."""
        idx = 1
        clauses = [f"event_type = ${idx}"]
        params: list[str | int] = [event_type]
        idx += 1
        if since is not None:
            clauses.append(f"timestamp >= ${idx}")
            params.append(since)
            idx += 1
        if until is not None:
            clauses.append(f"timestamp <= ${idx}")
            params.append(until)
            idx += 1
        if project is not None:
            clauses.append(f"payload->>'project' = ${idx}")
            params.append(project)
            idx += 1
        return " WHERE " + " AND ".join(clauses), params, idx

    async def query_summary(
        self,
        *,
        since: str | None = None,
        until: str | None = None,
        project: str | None = None,
    ) -> SummaryMetrics:
        """Return aggregated phase execution summary metrics."""
        where, params, _ = self._build_where(
            "PhaseCompleted", since=since, until=until, project=project
        )
        sql = (
            "SELECT"
            " COUNT(*) AS total,"
            " COUNT(*) FILTER (WHERE payload->>'outcome' = 'success'),"
            " COUNT(*) FILTER (WHERE payload->>'outcome' = 'fail'),"
            " COUNT(*) FILTER (WHERE payload->>'outcome' = 'error'),"
            " COUNT(*) FILTER (WHERE payload->>'outcome' = 'timeout'),"
            " COUNT(*) FILTER (WHERE payload->>'outcome' = 'blocked'),"
            " COALESCE(AVG((payload->>'duration_secs')::numeric), 0),"
            " COALESCE(PERCENTILE_CONT(0.5) WITHIN GROUP"
            " (ORDER BY (payload->>'duration_secs')::numeric), 0),"
            " COALESCE(PERCENTILE_CONT(0.95) WITHIN GROUP"
            " (ORDER BY (payload->>'duration_secs')::numeric), 0)"
            f" FROM events{where}"
        )
        row = await self._pool.fetchrow(sql, *params)
        if row is None or row[0] == 0:
            return SummaryMetrics()

        return SummaryMetrics(
            total_phases=row[0],
            succeeded=row[1],
            failed=row[2],
            errored=row[3],
            timed_out=row[4],
            blocked=row[5],
            avg_duration_secs=round(float(row[6]), 2),
            p50_duration_secs=round(float(row[7]), 2),
            p95_duration_secs=round(float(row[8]), 2),
        )

    async def query_costs(
        self,
        *,
        since: str | None = None,
        until: str | None = None,
        project: str | None = None,
        group_by: str = "model",
    ) -> CostMetrics:
        """Return token cost metrics grouped by model, day, or workflow."""
        where, params, _ = self._build_where(
            "TokenUsageRecorded", since=since, until=until, project=project
        )

        if group_by == "model":
            return await self._costs_by_model(where, params)
        elif group_by == "day":
            return await self._costs_grouped("LEFT(timestamp, 10)", where, params)
        else:  # workflow
            return await self._costs_grouped("workflow_id", where, params)

    async def _costs_by_model(self, where: str, params: list[str | int]) -> CostMetrics:
        """Fetch raw rows and aggregate by sorted models_used in Python."""
        sql = (
            "SELECT"
            " payload->'models_used',"
            " (payload->>'total_cost')::numeric,"
            " (payload->>'total_tokens')::bigint,"
            " (payload->>'input_tokens')::bigint,"
            " (payload->>'output_tokens')::bigint,"
            " COALESCE((payload->>'cache_read_tokens')::bigint, 0),"
            " COALESCE((payload->>'cache_creation_tokens')::bigint, 0),"
            " COALESCE((payload->>'reasoning_tokens')::bigint, 0)"
            f" FROM events{where}"
        )
        rows = await self._pool.fetch(sql, *params)

        groups: dict[str, dict[str, float | int]] = defaultdict(
            lambda: {
                "total_cost": 0.0,
                "total_tokens": 0,
                "input_tokens": 0,
                "output_tokens": 0,
                "cache_read_tokens": 0,
                "cache_creation_tokens": 0,
                "reasoning_tokens": 0,
                "event_count": 0,
            }
        )
        grand_cost = 0.0
        grand_tokens = 0

        for row in rows:
            models_raw = row[0]
            # asyncpg returns JSONB arrays as Python lists
            if isinstance(models_raw, str):
                try:
                    models = json.loads(models_raw)
                except json.JSONDecodeError, TypeError:
                    models = []
            elif isinstance(models_raw, list):
                models = models_raw
            else:
                models = []

            cost = float(row[1] or 0)
            tokens = int(row[2] or 0)
            key = _canonical_model_key(models)
            g = groups[key]
            g["total_cost"] += cost
            g["total_tokens"] += tokens
            g["input_tokens"] += int(row[3] or 0)
            g["output_tokens"] += int(row[4] or 0)
            g["cache_read_tokens"] += int(row[5] or 0)
            g["cache_creation_tokens"] += int(row[6] or 0)
            g["reasoning_tokens"] += int(row[7] or 0)
            g["event_count"] += 1
            grand_cost += cost
            grand_tokens += tokens

        buckets = [
            CostBucket(
                group_key=key,
                total_cost=round(g["total_cost"], 6),
                total_tokens=int(g["total_tokens"]),
                input_tokens=int(g["input_tokens"]),
                output_tokens=int(g["output_tokens"]),
                cache_read_tokens=int(g["cache_read_tokens"]),
                cache_creation_tokens=int(g["cache_creation_tokens"]),
                reasoning_tokens=int(g["reasoning_tokens"]),
                event_count=int(g["event_count"]),
            )
            for key, g in sorted(groups.items())
        ]

        return CostMetrics(
            buckets=buckets,
            total_cost=round(grand_cost, 6),
            total_tokens=grand_tokens,
        )

    async def _costs_grouped(
        self, group_expr: str, where: str, params: list[str | int]
    ) -> CostMetrics:
        """SQL GROUP BY aggregation for day or workflow grouping."""
        sql = (
            f"SELECT"
            f" {group_expr} AS group_key,"
            " SUM((payload->>'total_cost')::numeric),"
            " SUM((payload->>'total_tokens')::bigint),"
            " SUM((payload->>'input_tokens')::bigint),"
            " SUM((payload->>'output_tokens')::bigint),"
            " SUM(COALESCE((payload->>'cache_read_tokens')::bigint, 0)),"
            " SUM(COALESCE((payload->>'cache_creation_tokens')::bigint, 0)),"
            " SUM(COALESCE((payload->>'reasoning_tokens')::bigint, 0)),"
            " COUNT(*)"
            f" FROM events{where}"
            f" GROUP BY group_key ORDER BY group_key"
        )
        rows = await self._pool.fetch(sql, *params)

        grand_cost = 0.0
        grand_tokens = 0
        buckets: list[CostBucket] = []
        for row in rows:
            cost = float(row[1] or 0)
            tokens = int(row[2] or 0)
            grand_cost += cost
            grand_tokens += tokens
            buckets.append(
                CostBucket(
                    group_key=str(row[0]),
                    total_cost=round(cost, 6),
                    total_tokens=tokens,
                    input_tokens=int(row[3] or 0),
                    output_tokens=int(row[4] or 0),
                    cache_read_tokens=int(row[5] or 0),
                    cache_creation_tokens=int(row[6] or 0),
                    reasoning_tokens=int(row[7] or 0),
                    event_count=int(row[8]),
                )
            )

        return CostMetrics(
            buckets=buckets,
            total_cost=round(grand_cost, 6),
            total_tokens=grand_tokens,
        )

    async def query_vms(
        self,
        *,
        since: str | None = None,
        until: str | None = None,
        project: str | None = None,
    ) -> VMMetrics:
        """Return VM provisioning and utilization metrics."""
        prov_where, prov_params, prov_next_idx = self._build_where(
            "VMProvisioned", since=since, until=until, project=project
        )
        rel_where, rel_params, _ = self._build_where(
            "VMReleased", since=since, until=until, project=project
        )

        # Provisioned count and provider breakdown
        prov_sql = (
            "SELECT payload->>'provider' AS provider, COUNT(*)"
            f" FROM events{prov_where}"
            " GROUP BY provider"
        )
        prov_rows = await self._pool.fetch(prov_sql, *prov_params)
        total_provisioned = 0
        by_provider: dict[str, int] = {}
        for row in prov_rows:
            total_provisioned += row[1]
            by_provider[row[0] or "unknown"] = row[1]

        # Released stats
        rel_sql = (
            "SELECT"
            " COUNT(*),"
            " COALESCE(SUM((payload->>'duration_secs')::bigint), 0),"
            " COALESCE(SUM(COALESCE((payload->>'estimated_cost')::numeric, 0)), 0)"
            f" FROM events{rel_where}"
        )
        rel_row = await self._pool.fetchrow(rel_sql, *rel_params)
        total_released = rel_row[0] if rel_row else 0
        total_duration = int(rel_row[1]) if rel_row else 0
        total_cost = float(rel_row[2]) if rel_row else 0.0

        # Active VMs: provisioned in window without a matching release.
        # The release subquery respects until (as-of semantics) and project
        # filters so historical queries return correct active counts.
        release_clauses = [
            "r.event_type = 'VMReleased'",
            "r.payload->>'vm_id' = events.payload->>'vm_id'",
        ]
        release_params: list[str | int] = []
        idx = prov_next_idx
        if until is not None:
            release_clauses.append(f"r.timestamp <= ${idx}")
            release_params.append(until)
            idx += 1
        if project is not None:
            release_clauses.append(f"r.payload->>'project' = ${idx}")
            release_params.append(project)
            idx += 1
        release_where = " AND ".join(release_clauses)
        active_sql = (
            "SELECT COUNT(DISTINCT payload->>'vm_id')"
            f" FROM events{prov_where}"
            f" AND NOT EXISTS (SELECT 1 FROM events r WHERE {release_where})"
        )
        active_row = await self._pool.fetchval(active_sql, *prov_params, *release_params)
        currently_active = active_row or 0

        avg_dur = total_duration / max(total_released, 1) if total_released else 0.0

        return VMMetrics(
            total_provisioned=total_provisioned,
            total_released=total_released,
            currently_active=currently_active,
            total_vm_duration_secs=total_duration,
            total_estimated_cost=round(total_cost, 6),
            avg_duration_secs=round(avg_dur, 2),
            by_provider=by_provider,
        )
