# ruff: noqa: DOC201
"""SQLite-backed metrics reader for dashboard aggregation."""

from __future__ import annotations

import json
import logging
from collections import defaultdict
from pathlib import Path

import aiosqlite

from tanren_core.adapters.metrics_reader import (
    CostBucket,
    CostMetrics,
    SummaryMetrics,
    VMMetrics,
)

logger = logging.getLogger(__name__)


def _percentile(sorted_values: list[float], p: float) -> float:
    """Compute percentile from a pre-sorted list using linear interpolation."""
    if not sorted_values:
        return 0.0
    k = (len(sorted_values) - 1) * p
    f = int(k)
    c = f + 1
    if c >= len(sorted_values):
        return sorted_values[f]
    return sorted_values[f] + (k - f) * (sorted_values[c] - sorted_values[f])


def _canonical_model_key(models_used: list[str]) -> str:
    """Return a canonical JSON string key for a sorted model list."""
    return json.dumps(sorted(models_used))


class SqliteMetricsReader:
    """Reads aggregated metrics from a SQLite events database.

    Satisfies the MetricsReader protocol via structural typing.
    """

    def __init__(self, db_path: str | Path) -> None:
        """Initialize with the path to the SQLite events database."""
        self._db_path = Path(db_path)

    def _build_where(
        self,
        event_type: str,
        *,
        since: str | None,
        until: str | None,
        project: str | None,
        project_field: str = "$.project",
    ) -> tuple[str, list[str]]:
        """Build WHERE clause with filters. Returns (sql, params)."""
        clauses = ["event_type = ?"]
        params: list[str] = [event_type]
        if since is not None:
            clauses.append("timestamp >= ?")
            params.append(since)
        if until is not None:
            clauses.append("timestamp <= ?")
            params.append(until)
        if project is not None:
            clauses.append(f"json_extract(payload, '{project_field}') = ?")
            params.append(project)
        return " WHERE " + " AND ".join(clauses), params

    async def query_summary(
        self,
        *,
        since: str | None = None,
        until: str | None = None,
        project: str | None = None,
    ) -> SummaryMetrics:
        """Return aggregated phase execution summary metrics."""
        if not self._db_path.exists():
            return SummaryMetrics()

        where, params = self._build_where(
            "PhaseCompleted", since=since, until=until, project=project
        )

        async with aiosqlite.connect(f"file:{self._db_path}?mode=ro", uri=True) as conn:
            # Aggregate counts and average
            sql = (
                "SELECT"
                " COUNT(*) AS total,"
                " SUM(CASE WHEN json_extract(payload, '$.outcome') = 'success' THEN 1 ELSE 0 END),"
                " SUM(CASE WHEN json_extract(payload, '$.outcome') = 'fail' THEN 1 ELSE 0 END),"
                " SUM(CASE WHEN json_extract(payload, '$.outcome') = 'error' THEN 1 ELSE 0 END),"
                " SUM(CASE WHEN json_extract(payload, '$.outcome') = 'timeout' THEN 1 ELSE 0 END),"
                " SUM(CASE WHEN json_extract(payload, '$.outcome') = 'blocked' THEN 1 ELSE 0 END),"
                " AVG(CAST(json_extract(payload, '$.duration_secs') AS REAL))"
                f" FROM events{where}"
            )
            cursor = await conn.execute(sql, params)
            row = await cursor.fetchone()
            if row is None or row[0] == 0:
                return SummaryMetrics()

            total, succeeded, failed, errored, timed_out, blocked, avg_dur = row

            # Fetch sorted durations for percentile computation
            dur_sql = (
                "SELECT CAST(json_extract(payload, '$.duration_secs') AS REAL) AS dur"
                f" FROM events{where} ORDER BY dur"
            )
            cursor = await conn.execute(dur_sql, params)
            durations = [r[0] for r in await cursor.fetchall()]

        return SummaryMetrics(
            total_phases=total,
            succeeded=succeeded or 0,
            failed=failed or 0,
            errored=errored or 0,
            timed_out=timed_out or 0,
            blocked=blocked or 0,
            avg_duration_secs=round(avg_dur or 0.0, 2),
            p50_duration_secs=round(_percentile(durations, 0.5), 2),
            p95_duration_secs=round(_percentile(durations, 0.95), 2),
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
        if not self._db_path.exists():
            return CostMetrics()

        where, params = self._build_where(
            "TokenUsageRecorded", since=since, until=until, project=project
        )

        if group_by == "model":
            return await self._costs_by_model(where, params)
        elif group_by == "day":
            return await self._costs_grouped("substr(timestamp, 1, 10)", where, params)
        else:  # workflow
            return await self._costs_grouped("workflow_id", where, params)

    async def _costs_by_model(self, where: str, params: list[str]) -> CostMetrics:
        """Fetch raw rows and aggregate by sorted models_used in Python."""
        sql = (
            "SELECT"
            " json_extract(payload, '$.models_used'),"
            " CAST(json_extract(payload, '$.total_cost') AS REAL),"
            " CAST(json_extract(payload, '$.total_tokens') AS INTEGER),"
            " CAST(json_extract(payload, '$.input_tokens') AS INTEGER),"
            " CAST(json_extract(payload, '$.output_tokens') AS INTEGER),"
            " CAST(COALESCE(json_extract(payload, '$.cache_read_tokens'), '0') AS INTEGER),"
            " CAST(COALESCE(json_extract(payload, '$.cache_creation_tokens'), '0') AS INTEGER),"
            " CAST(COALESCE(json_extract(payload, '$.reasoning_tokens'), '0') AS INTEGER)"
            f" FROM events{where}"
        )
        async with aiosqlite.connect(f"file:{self._db_path}?mode=ro", uri=True) as conn:
            cursor = await conn.execute(sql, params)
            rows = await cursor.fetchall()

        # Aggregate in Python by canonical model key
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
            models_json, cost, tokens, inp, out, cache_r, cache_c, reasoning = row
            try:
                models = (
                    json.loads(models_json) if isinstance(models_json, str) else (models_json or [])
                )
            except json.JSONDecodeError, TypeError:
                models = []
            key = _canonical_model_key(models)
            g = groups[key]
            g["total_cost"] += cost or 0.0
            g["total_tokens"] += tokens or 0
            g["input_tokens"] += inp or 0
            g["output_tokens"] += out or 0
            g["cache_read_tokens"] += cache_r or 0
            g["cache_creation_tokens"] += cache_c or 0
            g["reasoning_tokens"] += reasoning or 0
            g["event_count"] += 1
            grand_cost += cost or 0.0
            grand_tokens += tokens or 0

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

    async def _costs_grouped(self, group_expr: str, where: str, params: list[str]) -> CostMetrics:
        """SQL GROUP BY aggregation for day or workflow grouping."""
        sql = (
            f"SELECT"
            f" {group_expr} AS group_key,"
            " SUM(CAST(json_extract(payload, '$.total_cost') AS REAL)),"
            " SUM(CAST(json_extract(payload, '$.total_tokens') AS INTEGER)),"
            " SUM(CAST(json_extract(payload, '$.input_tokens') AS INTEGER)),"
            " SUM(CAST(json_extract(payload, '$.output_tokens') AS INTEGER)),"
            " SUM(CAST(COALESCE(json_extract(payload, '$.cache_read_tokens'), '0') AS INTEGER)),"
            " SUM(CAST(COALESCE("
            "json_extract(payload, '$.cache_creation_tokens'), '0') AS INTEGER)),"
            " SUM(CAST(COALESCE(json_extract(payload, '$.reasoning_tokens'), '0') AS INTEGER)),"
            " COUNT(*)"
            f" FROM events{where}"
            f" GROUP BY group_key ORDER BY group_key"
        )
        async with aiosqlite.connect(f"file:{self._db_path}?mode=ro", uri=True) as conn:
            cursor = await conn.execute(sql, params)
            rows = await cursor.fetchall()

        grand_cost = 0.0
        grand_tokens = 0
        buckets: list[CostBucket] = []
        for row in rows:
            key, cost, tokens, inp, out, cache_r, cache_c, reasoning, count = row
            cost = cost or 0.0
            tokens = tokens or 0
            grand_cost += cost
            grand_tokens += tokens
            buckets.append(
                CostBucket(
                    group_key=str(key),
                    total_cost=round(cost, 6),
                    total_tokens=tokens,
                    input_tokens=inp or 0,
                    output_tokens=out or 0,
                    cache_read_tokens=cache_r or 0,
                    cache_creation_tokens=cache_c or 0,
                    reasoning_tokens=reasoning or 0,
                    event_count=count,
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
        if not self._db_path.exists():
            return VMMetrics()

        prov_where, prov_params = self._build_where(
            "VMProvisioned", since=since, until=until, project=project
        )
        rel_where, rel_params = self._build_where(
            "VMReleased", since=since, until=until, project=project
        )

        async with aiosqlite.connect(f"file:{self._db_path}?mode=ro", uri=True) as conn:
            # Provisioned count and provider breakdown
            prov_sql = (
                "SELECT json_extract(payload, '$.provider') AS provider, COUNT(*)"
                f" FROM events{prov_where}"
                " GROUP BY provider"
            )
            cursor = await conn.execute(prov_sql, prov_params)
            prov_rows = await cursor.fetchall()
            total_provisioned = 0
            by_provider: dict[str, int] = {}
            for provider, count in prov_rows:
                total_provisioned += count
                by_provider[provider or "unknown"] = count

            # Released stats
            rel_sql = (
                "SELECT"
                " COUNT(*),"
                " COALESCE(SUM(CAST(json_extract(payload, '$.duration_secs') AS INTEGER)), 0),"
                " COALESCE(SUM(CAST(COALESCE("
                "json_extract(payload, '$.estimated_cost'), '0') AS REAL)), 0)"
                f" FROM events{rel_where}"
            )
            cursor = await conn.execute(rel_sql, rel_params)
            rel_row = await cursor.fetchone()
            total_released = rel_row[0] if rel_row else 0
            total_duration = rel_row[1] if rel_row else 0
            total_cost = rel_row[2] if rel_row else 0.0

            # Active VMs: provisioned in window without a matching release
            # The release subquery respects until (as-of semantics) and project
            # filters so historical queries return correct active counts.
            release_clauses = [
                "r.event_type = 'VMReleased'",
                "json_extract(r.payload, '$.vm_id') = json_extract(events.payload, '$.vm_id')",
            ]
            release_params: list[str] = []
            if until is not None:
                release_clauses.append("r.timestamp <= ?")
                release_params.append(until)
            if project is not None:
                release_clauses.append("json_extract(r.payload, '$.project') = ?")
                release_params.append(project)
            release_where = " AND ".join(release_clauses)
            active_sql = (
                "SELECT COUNT(DISTINCT json_extract(payload, '$.vm_id'))"
                f" FROM events{prov_where}"
                f" AND NOT EXISTS (SELECT 1 FROM events r WHERE {release_where})"
            )
            cursor = await conn.execute(active_sql, [*prov_params, *release_params])
            active_row = await cursor.fetchone()
            currently_active = active_row[0] if active_row else 0

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
