"""Tests for the Postgres metrics reader."""

from __future__ import annotations

from unittest.mock import AsyncMock, MagicMock

from tanren_core.adapters.metrics_reader import MetricsReader
from tanren_core.adapters.postgres_metrics_reader import PostgresMetricsReader


def _mock_pool():
    pool = MagicMock()
    pool.fetchrow = AsyncMock(return_value=None)
    pool.fetch = AsyncMock(return_value=[])
    pool.fetchval = AsyncMock(return_value=0)
    return pool


class TestQuerySummary:
    async def test_no_filters(self):
        pool = _mock_pool()
        pool.fetchrow = AsyncMock(return_value=[0, 0, 0, 0, 0, 0, 0, 0, 0])
        reader = PostgresMetricsReader(pool)

        result = await reader.query_summary()

        assert result.total_phases == 0
        sql = pool.fetchrow.call_args[0][0]
        assert "event_type = $1" in sql
        assert "PERCENTILE_CONT" in sql

    async def test_with_all_filters(self):
        pool = _mock_pool()
        pool.fetchrow = AsyncMock(return_value=[5, 3, 1, 1, 0, 0, 30.0, 25.0, 48.0])
        reader = PostgresMetricsReader(pool)

        result = await reader.query_summary(
            since="2026-03-01T00:00:00Z",
            until="2026-03-31T00:00:00Z",
            project="alpha",
        )

        assert result.total_phases == 5
        assert result.succeeded == 3
        args = pool.fetchrow.call_args[0]
        # $1=event_type, $2=since, $3=until, $4=project
        assert args[1] == "PhaseCompleted"
        assert args[2] == "2026-03-01T00:00:00Z"
        assert args[3] == "2026-03-31T00:00:00Z"
        assert args[4] == "alpha"
        sql = args[0]
        assert "timestamp >= $2" in sql
        assert "timestamp <= $3" in sql
        assert "payload->>'project' = $4" in sql


class TestQueryCosts:
    async def test_group_by_day(self):
        pool = _mock_pool()
        pool.fetch = AsyncMock(
            return_value=[
                ["2026-03-01", 0.15, 4500, 3000, 1500, 0, 0, 0, 2],
                ["2026-03-02", 0.02, 700, 500, 200, 0, 0, 0, 1],
            ]
        )
        reader = PostgresMetricsReader(pool)

        result = await reader.query_costs(group_by="day")

        assert len(result.buckets) == 2
        assert result.buckets[0].group_key == "2026-03-01"
        sql = pool.fetch.call_args[0][0]
        assert "LEFT(timestamp, 10)" in sql
        assert "GROUP BY" in sql

    async def test_group_by_model(self):
        pool = _mock_pool()
        # Model grouping fetches raw rows
        pool.fetch = AsyncMock(
            return_value=[
                [["claude-sonnet-4-20250514"], 0.05, 1500, 1000, 500, 0, 0, 0],
                [["claude-opus-4-6", "claude-sonnet-4-20250514"], 0.10, 3000, 2000, 1000, 0, 0, 0],
            ]
        )
        reader = PostgresMetricsReader(pool)

        result = await reader.query_costs(group_by="model")

        assert len(result.buckets) == 2
        # Model keys should be canonical (sorted JSON)
        keys = [b.group_key for b in result.buckets]
        assert '["claude-opus-4-6", "claude-sonnet-4-20250514"]' in keys
        assert '["claude-sonnet-4-20250514"]' in keys
        # No GROUP BY in SQL for model grouping
        sql = pool.fetch.call_args[0][0]
        assert "GROUP BY" not in sql

    async def test_group_by_workflow(self):
        pool = _mock_pool()
        pool.fetch = AsyncMock(return_value=[])
        reader = PostgresMetricsReader(pool)

        await reader.query_costs(group_by="workflow")

        sql = pool.fetch.call_args[0][0]
        assert "workflow_id" in sql
        assert "GROUP BY" in sql


class TestQueryVMs:
    async def test_queries_structure(self):
        pool = _mock_pool()
        pool.fetch = AsyncMock(return_value=[])
        pool.fetchrow = AsyncMock(return_value=[0, 0, 0])
        pool.fetchval = AsyncMock(return_value=0)
        reader = PostgresMetricsReader(pool)

        result = await reader.query_vms()

        assert result.total_provisioned == 0
        assert result.total_released == 0
        assert result.currently_active == 0
        # Should have made calls for provisioned, released, and active
        assert pool.fetch.await_count == 1  # provider breakdown
        assert pool.fetchrow.await_count == 1  # released stats
        assert pool.fetchval.await_count == 1  # active count


class TestProtocol:
    def test_isinstance(self):
        pool = _mock_pool()
        reader = PostgresMetricsReader(pool)
        assert isinstance(reader, MetricsReader)
