"""Tests for metrics endpoints."""

from __future__ import annotations

import json

import aiosqlite
import pytest

from tanren_core.adapters.sqlite_emitter import _SCHEMA  # noqa: PLC2701
from tanren_core.adapters.sqlite_metrics_reader import SqliteMetricsReader


async def _setup_db(db_path, events: list[tuple[str, str, str, dict]]):
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


FIXTURE_EVENTS: list[tuple[str, str, str, dict]] = [
    # PhaseCompleted — 2 success, 1 fail
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
            "duration_secs": 100,
            "exit_code": 0,
        },
    ),
    (
        "2026-03-01T12:00:00Z",
        "wf-alpha-2-200",
        "PhaseCompleted",
        {
            "type": "phase_completed",
            "timestamp": "2026-03-01T12:00:00Z",
            "workflow_id": "wf-alpha-2-200",
            "phase": "do-task",
            "project": "alpha",
            "outcome": "success",
            "duration_secs": 200,
            "exit_code": 0,
        },
    ),
    (
        "2026-03-02T10:00:00Z",
        "wf-beta-1-300",
        "PhaseCompleted",
        {
            "type": "phase_completed",
            "timestamp": "2026-03-02T10:00:00Z",
            "workflow_id": "wf-beta-1-300",
            "phase": "do-task",
            "project": "beta",
            "outcome": "fail",
            "duration_secs": 300,
            "exit_code": 1,
        },
    ),
    # TokenUsageRecorded
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
    # VMProvisioned + VMReleased
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
    # VMProvisioned without release (active)
    (
        "2026-03-02T09:55:00Z",
        "wf-beta-1-300",
        "VMProvisioned",
        {
            "type": "vm_provisioned",
            "timestamp": "2026-03-02T09:55:00Z",
            "workflow_id": "wf-beta-1-300",
            "vm_id": "vm-2",
            "host": "10.0.0.2",
            "provider": "gcp",
            "project": "beta",
            "profile": "gpu",
            "hourly_cost": 2.00,
        },
    ),
]


@pytest.mark.api
class TestMetricsSummary:
    async def test_no_reader_returns_zeros(self, client, auth_headers):
        resp = await client.get("/api/v1/metrics/summary", headers=auth_headers)
        assert resp.status_code == 200
        data = resp.json()
        assert data["total_phases"] == 0
        assert data["success_rate"] == pytest.approx(0.0)

    async def test_with_data(self, client, auth_headers, app, tmp_path):
        db = tmp_path / "events.db"
        await _setup_db(db, FIXTURE_EVENTS)
        app.state.metrics_reader = SqliteMetricsReader(db)

        resp = await client.get("/api/v1/metrics/summary", headers=auth_headers)
        assert resp.status_code == 200
        data = resp.json()
        assert data["total_phases"] == 3
        assert data["succeeded"] == 2
        assert data["failed"] == 1
        assert data["success_rate"] == pytest.approx(0.6667)

    async def test_project_filter(self, client, auth_headers, app, tmp_path):
        db = tmp_path / "events.db"
        await _setup_db(db, FIXTURE_EVENTS)
        app.state.metrics_reader = SqliteMetricsReader(db)

        resp = await client.get("/api/v1/metrics/summary?project=alpha", headers=auth_headers)
        data = resp.json()
        assert data["total_phases"] == 2
        assert data["succeeded"] == 2


@pytest.mark.api
class TestMetricsCosts:
    async def test_group_by_model(self, client, auth_headers, app, tmp_path):
        db = tmp_path / "events.db"
        await _setup_db(db, FIXTURE_EVENTS)
        app.state.metrics_reader = SqliteMetricsReader(db)

        resp = await client.get("/api/v1/metrics/costs", headers=auth_headers)
        assert resp.status_code == 200
        data = resp.json()
        assert data["group_by"] == "model"
        assert len(data["buckets"]) == 1
        assert data["total_cost"] == pytest.approx(0.05)

    async def test_group_by_day(self, client, auth_headers, app, tmp_path):
        db = tmp_path / "events.db"
        await _setup_db(db, FIXTURE_EVENTS)
        app.state.metrics_reader = SqliteMetricsReader(db)

        resp = await client.get("/api/v1/metrics/costs?group_by=day", headers=auth_headers)
        data = resp.json()
        assert data["group_by"] == "day"
        assert len(data["buckets"]) == 1
        assert data["buckets"][0]["group_key"] == "2026-03-01"

    async def test_invalid_group_by(self, client, auth_headers):
        resp = await client.get("/api/v1/metrics/costs?group_by=invalid", headers=auth_headers)
        assert resp.status_code == 422


@pytest.mark.api
class TestMetricsVMs:
    async def test_active_count(self, client, auth_headers, app, tmp_path):
        db = tmp_path / "events.db"
        await _setup_db(db, FIXTURE_EVENTS)
        app.state.metrics_reader = SqliteMetricsReader(db)

        resp = await client.get("/api/v1/metrics/vms", headers=auth_headers)
        assert resp.status_code == 200
        data = resp.json()
        assert data["total_provisioned"] == 2
        assert data["total_released"] == 1
        assert data["currently_active"] == 1
        assert data["by_provider"] == {"hetzner": 1, "gcp": 1}

    async def test_no_reader_returns_zeros(self, client, auth_headers):
        resp = await client.get("/api/v1/metrics/vms", headers=auth_headers)
        assert resp.status_code == 200
        data = resp.json()
        assert data["total_provisioned"] == 0


@pytest.mark.api
class TestMetricsAuth:
    async def test_missing_auth(self, client):
        resp = await client.get("/api/v1/metrics/summary")
        assert resp.status_code == 422
