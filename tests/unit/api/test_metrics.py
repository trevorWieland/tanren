"""Tests for metrics endpoints."""

from __future__ import annotations

import pytest

from tanren_core.adapters.events import (
    PhaseCompleted,
    TokenUsageRecorded,
    VMProvisioned,
    VMReleased,
)
from tanren_core.adapters.remote_types import VMProvider


async def _insert_fixture_events(store) -> None:
    """Insert standard fixture events into the store."""
    # PhaseCompleted — 2 success, 1 fail
    await store.append(
        PhaseCompleted(
            timestamp="2026-03-01T10:00:00Z",
            workflow_id="wf-alpha-1-100",
            phase="do-task",
            project="alpha",
            outcome="success",
            duration_secs=100,
            exit_code=0,
        )
    )
    await store.append(
        PhaseCompleted(
            timestamp="2026-03-01T12:00:00Z",
            workflow_id="wf-alpha-2-200",
            phase="do-task",
            project="alpha",
            outcome="success",
            duration_secs=200,
            exit_code=0,
        )
    )
    await store.append(
        PhaseCompleted(
            timestamp="2026-03-02T10:00:00Z",
            workflow_id="wf-beta-1-300",
            phase="do-task",
            project="beta",
            outcome="fail",
            duration_secs=300,
            exit_code=1,
        )
    )
    # TokenUsageRecorded
    await store.append(
        TokenUsageRecorded(
            timestamp="2026-03-01T10:05:00Z",
            workflow_id="wf-alpha-1-100",
            phase="do-task",
            project="alpha",
            cli="claude",
            input_tokens=1000,
            output_tokens=500,
            total_tokens=1500,
            total_cost=0.05,
            models_used=["claude-sonnet-4-20250514"],
        )
    )
    # VMProvisioned + VMReleased
    await store.append(
        VMProvisioned(
            timestamp="2026-03-01T09:55:00Z",
            workflow_id="wf-alpha-1-100",
            vm_id="vm-1",
            host="10.0.0.1",
            provider=VMProvider.HETZNER,
            project="alpha",
            profile="default",
            hourly_cost=0.50,
        )
    )
    await store.append(
        VMReleased(
            timestamp="2026-03-01T10:05:00Z",
            workflow_id="wf-alpha-1-100",
            vm_id="vm-1",
            project="alpha",
            duration_secs=600,
            estimated_cost=0.083,
        )
    )
    # VMProvisioned without release (active)
    await store.append(
        VMProvisioned(
            timestamp="2026-03-02T09:55:00Z",
            workflow_id="wf-beta-1-300",
            vm_id="vm-2",
            host="10.0.0.2",
            provider=VMProvider.GCP,
            project="beta",
            profile="gpu",
            hourly_cost=2.00,
        )
    )


@pytest.mark.api
class TestMetricsSummary:
    async def test_no_data_returns_zeros(self, client, auth_headers):
        resp = await client.get("/api/v1/metrics/summary", headers=auth_headers)
        assert resp.status_code == 200
        data = resp.json()
        assert data["total_phases"] == 0
        assert data["success_rate"] == pytest.approx(0.0)

    async def test_with_data(self, client, auth_headers, sqlite_store):
        await _insert_fixture_events(sqlite_store)

        resp = await client.get("/api/v1/metrics/summary", headers=auth_headers)
        assert resp.status_code == 200
        data = resp.json()
        assert data["total_phases"] == 3
        assert data["succeeded"] == 2
        assert data["failed"] == 1
        assert data["success_rate"] == pytest.approx(0.6667)

    async def test_project_filter(self, client, auth_headers, sqlite_store):
        await _insert_fixture_events(sqlite_store)

        resp = await client.get("/api/v1/metrics/summary?project=alpha", headers=auth_headers)
        data = resp.json()
        assert data["total_phases"] == 2
        assert data["succeeded"] == 2


@pytest.mark.api
class TestMetricsCosts:
    async def test_group_by_model(self, client, auth_headers, sqlite_store):
        await _insert_fixture_events(sqlite_store)

        resp = await client.get("/api/v1/metrics/costs", headers=auth_headers)
        assert resp.status_code == 200
        data = resp.json()
        assert data["group_by"] == "model"
        assert len(data["buckets"]) >= 1
        assert data["total_cost"] == pytest.approx(0.05)

    async def test_group_by_day(self, client, auth_headers, sqlite_store):
        await _insert_fixture_events(sqlite_store)

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
    async def test_active_count(self, client, auth_headers, sqlite_store):
        await _insert_fixture_events(sqlite_store)

        resp = await client.get("/api/v1/metrics/vms", headers=auth_headers)
        assert resp.status_code == 200
        data = resp.json()
        assert data["total_provisioned"] == 2
        assert data["total_released"] == 1
        assert data["currently_active"] == 1
        assert data["by_provider"] == {"hetzner": 1, "gcp": 1}

    async def test_no_data_returns_zeros(self, client, auth_headers):
        resp = await client.get("/api/v1/metrics/vms", headers=auth_headers)
        assert resp.status_code == 200
        data = resp.json()
        assert data["total_provisioned"] == 0


@pytest.mark.api
class TestMetricsAuth:
    async def test_missing_auth(self, client):
        resp = await client.get("/api/v1/metrics/summary")
        assert resp.status_code == 422
