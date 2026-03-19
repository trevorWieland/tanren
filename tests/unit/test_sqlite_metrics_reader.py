"""Tests for SqliteMetricsReader."""

from __future__ import annotations

import json
from pathlib import Path

import aiosqlite
import pytest

from tanren_core.adapters.metrics_reader import MetricsReader
from tanren_core.adapters.sqlite_emitter import _SCHEMA  # noqa: PLC2701
from tanren_core.adapters.sqlite_metrics_reader import SqliteMetricsReader


async def _setup_db(db_path: Path, events: list[tuple[str, str, str, dict]]) -> None:
    """Create DB with schema and insert events."""
    async with aiosqlite.connect(str(db_path)) as conn:
        await conn.executescript(_SCHEMA)
        for ts, wid, etype, payload in events:
            await conn.execute(
                "INSERT INTO events"
                " (timestamp, workflow_id, event_type, payload)"
                " VALUES (?, ?, ?, ?)",
                (ts, wid, etype, json.dumps(payload)),
            )
        await conn.commit()


# ---------------------------------------------------------------------------
# Fixture data
# ---------------------------------------------------------------------------

PHASE_EVENTS: list[tuple[str, str, str, dict]] = [
    (
        "2026-03-01T10:00:00Z",
        "wf-alpha-1-100",
        "PhaseCompleted",
        {
            "type": "phase_completed",
            "timestamp": "2026-03-01T10:00:00Z",
            "workflow_id": "wf-alpha-1-100",
            "phase": "do-task",
            "project": "alpha",
            "outcome": "success",
            "duration_secs": 10,
            "exit_code": 0,
        },
    ),
    (
        "2026-03-01T11:00:00Z",
        "wf-alpha-2-200",
        "PhaseCompleted",
        {
            "type": "phase_completed",
            "timestamp": "2026-03-01T11:00:00Z",
            "workflow_id": "wf-alpha-2-200",
            "phase": "do-task",
            "project": "alpha",
            "outcome": "success",
            "duration_secs": 20,
            "exit_code": 0,
        },
    ),
    (
        "2026-03-02T10:00:00Z",
        "wf-alpha-3-300",
        "PhaseCompleted",
        {
            "type": "phase_completed",
            "timestamp": "2026-03-02T10:00:00Z",
            "workflow_id": "wf-alpha-3-300",
            "phase": "audit-task",
            "project": "alpha",
            "outcome": "success",
            "duration_secs": 30,
            "exit_code": 0,
        },
    ),
    (
        "2026-03-02T11:00:00Z",
        "wf-beta-1-400",
        "PhaseCompleted",
        {
            "type": "phase_completed",
            "timestamp": "2026-03-02T11:00:00Z",
            "workflow_id": "wf-beta-1-400",
            "phase": "do-task",
            "project": "beta",
            "outcome": "fail",
            "duration_secs": 40,
            "exit_code": 1,
        },
    ),
    (
        "2026-03-03T10:00:00Z",
        "wf-beta-2-500",
        "PhaseCompleted",
        {
            "type": "phase_completed",
            "timestamp": "2026-03-03T10:00:00Z",
            "workflow_id": "wf-beta-2-500",
            "phase": "do-task",
            "project": "beta",
            "outcome": "error",
            "duration_secs": 50,
            "exit_code": 1,
        },
    ),
]

TOKEN_EVENTS: list[tuple[str, str, str, dict]] = [
    (
        "2026-03-01T10:05:00Z",
        "wf-alpha-1-100",
        "TokenUsageRecorded",
        {
            "type": "token_usage_recorded",
            "timestamp": "2026-03-01T10:05:00Z",
            "workflow_id": "wf-alpha-1-100",
            "phase": "do-task",
            "project": "alpha",
            "cli": "claude",
            "input_tokens": 1000,
            "output_tokens": 500,
            "total_tokens": 1500,
            "total_cost": 0.05,
            "models_used": ["claude-sonnet-4-20250514"],
        },
    ),
    (
        "2026-03-01T11:05:00Z",
        "wf-alpha-2-200",
        "TokenUsageRecorded",
        {
            "type": "token_usage_recorded",
            "timestamp": "2026-03-01T11:05:00Z",
            "workflow_id": "wf-alpha-2-200",
            "phase": "do-task",
            "project": "alpha",
            "cli": "claude",
            "input_tokens": 2000,
            "output_tokens": 1000,
            "total_tokens": 3000,
            "total_cost": 0.10,
            "models_used": ["claude-sonnet-4-20250514", "claude-opus-4-6"],
        },
    ),
    (
        "2026-03-02T10:05:00Z",
        "wf-alpha-3-300",
        "TokenUsageRecorded",
        {
            "type": "token_usage_recorded",
            "timestamp": "2026-03-02T10:05:00Z",
            "workflow_id": "wf-alpha-3-300",
            "phase": "audit-task",
            "project": "alpha",
            "cli": "claude",
            "input_tokens": 500,
            "output_tokens": 200,
            "total_tokens": 700,
            "total_cost": 0.02,
            "models_used": ["claude-sonnet-4-20250514"],
        },
    ),
]

VM_EVENTS: list[tuple[str, str, str, dict]] = [
    (
        "2026-03-01T09:55:00Z",
        "wf-alpha-1-100",
        "VMProvisioned",
        {
            "type": "vm_provisioned",
            "timestamp": "2026-03-01T09:55:00Z",
            "workflow_id": "wf-alpha-1-100",
            "vm_id": "vm-1",
            "host": "10.0.0.1",
            "provider": "hetzner",
            "project": "alpha",
            "profile": "default",
            "hourly_cost": 0.50,
        },
    ),
    (
        "2026-03-01T10:05:00Z",
        "wf-alpha-1-100",
        "VMReleased",
        {
            "type": "vm_released",
            "timestamp": "2026-03-01T10:05:00Z",
            "workflow_id": "wf-alpha-1-100",
            "vm_id": "vm-1",
            "project": "alpha",
            "duration_secs": 600,
            "estimated_cost": 0.083,
        },
    ),
    (
        "2026-03-02T09:55:00Z",
        "wf-alpha-3-300",
        "VMProvisioned",
        {
            "type": "vm_provisioned",
            "timestamp": "2026-03-02T09:55:00Z",
            "workflow_id": "wf-alpha-3-300",
            "vm_id": "vm-2",
            "host": "10.0.0.2",
            "provider": "gcp",
            "project": "alpha",
            "profile": "gpu",
            "hourly_cost": 2.00,
        },
    ),
    # vm-2 is NOT released — should appear as active
]

ALL_EVENTS = PHASE_EVENTS + TOKEN_EVENTS + VM_EVENTS


# ---------------------------------------------------------------------------
# Summary tests
# ---------------------------------------------------------------------------


class TestQuerySummary:
    @pytest.mark.asyncio
    async def test_all_events(self, tmp_path: Path):
        db = tmp_path / "events.db"
        await _setup_db(db, ALL_EVENTS)
        reader = SqliteMetricsReader(db)
        result = await reader.query_summary()

        assert result.total_phases == 5
        assert result.succeeded == 3
        assert result.failed == 1
        assert result.errored == 1
        assert result.timed_out == 0
        assert result.blocked == 0
        assert result.avg_duration_secs == pytest.approx(30.0)
        # Sorted durations: [10, 20, 30, 40, 50]
        assert result.p50_duration_secs == pytest.approx(30.0)
        assert result.p95_duration_secs == pytest.approx(48.0)

    @pytest.mark.asyncio
    async def test_project_filter(self, tmp_path: Path):
        db = tmp_path / "events.db"
        await _setup_db(db, ALL_EVENTS)
        reader = SqliteMetricsReader(db)
        result = await reader.query_summary(project="alpha")

        assert result.total_phases == 3
        assert result.succeeded == 3
        assert result.failed == 0

    @pytest.mark.asyncio
    async def test_time_range(self, tmp_path: Path):
        db = tmp_path / "events.db"
        await _setup_db(db, ALL_EVENTS)
        reader = SqliteMetricsReader(db)
        result = await reader.query_summary(since="2026-03-02T00:00:00Z")

        assert result.total_phases == 3
        assert result.succeeded == 1
        assert result.failed == 1
        assert result.errored == 1

    @pytest.mark.asyncio
    async def test_empty_db(self, tmp_path: Path):
        db = tmp_path / "events.db"
        await _setup_db(db, [])
        reader = SqliteMetricsReader(db)
        result = await reader.query_summary()
        assert result.total_phases == 0

    @pytest.mark.asyncio
    async def test_nonexistent_db(self, tmp_path: Path):
        reader = SqliteMetricsReader(tmp_path / "missing.db")
        result = await reader.query_summary()
        assert result.total_phases == 0


# ---------------------------------------------------------------------------
# Cost tests
# ---------------------------------------------------------------------------


class TestQueryCosts:
    @pytest.mark.asyncio
    async def test_group_by_model(self, tmp_path: Path):
        db = tmp_path / "events.db"
        await _setup_db(db, ALL_EVENTS)
        reader = SqliteMetricsReader(db)
        result = await reader.query_costs(group_by="model")

        assert len(result.buckets) == 2
        # Sorted keys: multi-model first (alphabetically)
        keys = [b.group_key for b in result.buckets]
        # ["claude-opus-4-6", "claude-sonnet-4-20250514"] sorts before ["claude-sonnet-4-20250514"]
        assert '["claude-opus-4-6", "claude-sonnet-4-20250514"]' in keys
        assert '["claude-sonnet-4-20250514"]' in keys

        # The multi-model bucket has 1 event with cost 0.10
        multi = next(b for b in result.buckets if "opus" in b.group_key)
        assert multi.event_count == 1
        assert multi.total_cost == pytest.approx(0.10)

        # The single-model bucket has 2 events with cost 0.05 + 0.02 = 0.07
        single = next(b for b in result.buckets if b.group_key == '["claude-sonnet-4-20250514"]')
        assert single.event_count == 2
        assert single.total_cost == pytest.approx(0.07)

        assert result.total_cost == pytest.approx(0.17)
        assert result.total_tokens == 5200

    @pytest.mark.asyncio
    async def test_group_by_day(self, tmp_path: Path):
        db = tmp_path / "events.db"
        await _setup_db(db, ALL_EVENTS)
        reader = SqliteMetricsReader(db)
        result = await reader.query_costs(group_by="day")

        assert len(result.buckets) == 2
        assert result.buckets[0].group_key == "2026-03-01"
        assert result.buckets[0].event_count == 2
        assert result.buckets[1].group_key == "2026-03-02"
        assert result.buckets[1].event_count == 1

    @pytest.mark.asyncio
    async def test_group_by_workflow(self, tmp_path: Path):
        db = tmp_path / "events.db"
        await _setup_db(db, ALL_EVENTS)
        reader = SqliteMetricsReader(db)
        result = await reader.query_costs(group_by="workflow")

        assert len(result.buckets) == 3

    @pytest.mark.asyncio
    async def test_project_filter(self, tmp_path: Path):
        db = tmp_path / "events.db"
        await _setup_db(db, ALL_EVENTS)
        reader = SqliteMetricsReader(db)
        result = await reader.query_costs(project="alpha")
        # All 3 token events are project alpha
        assert result.total_tokens == 5200

    @pytest.mark.asyncio
    async def test_empty(self, tmp_path: Path):
        db = tmp_path / "events.db"
        await _setup_db(db, [])
        reader = SqliteMetricsReader(db)
        result = await reader.query_costs()
        assert result.buckets == []
        assert result.total_cost == pytest.approx(0.0)


# ---------------------------------------------------------------------------
# VM tests
# ---------------------------------------------------------------------------


class TestQueryVMs:
    @pytest.mark.asyncio
    async def test_all(self, tmp_path: Path):
        db = tmp_path / "events.db"
        await _setup_db(db, ALL_EVENTS)
        reader = SqliteMetricsReader(db)
        result = await reader.query_vms()

        assert result.total_provisioned == 2
        assert result.total_released == 1
        assert result.currently_active == 1
        assert result.total_vm_duration_secs == 600
        assert result.total_estimated_cost == pytest.approx(0.083)
        assert result.avg_duration_secs == pytest.approx(600.0)

    @pytest.mark.asyncio
    async def test_provider_breakdown(self, tmp_path: Path):
        db = tmp_path / "events.db"
        await _setup_db(db, ALL_EVENTS)
        reader = SqliteMetricsReader(db)
        result = await reader.query_vms()

        assert result.by_provider == {"hetzner": 1, "gcp": 1}

    @pytest.mark.asyncio
    async def test_empty(self, tmp_path: Path):
        db = tmp_path / "events.db"
        await _setup_db(db, [])
        reader = SqliteMetricsReader(db)
        result = await reader.query_vms()
        assert result.total_provisioned == 0
        assert result.currently_active == 0


# ---------------------------------------------------------------------------
# Protocol conformance
# ---------------------------------------------------------------------------


class TestProtocol:
    def test_isinstance(self, tmp_path: Path):
        reader = SqliteMetricsReader(tmp_path / "x.db")
        assert isinstance(reader, MetricsReader)
