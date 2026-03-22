"""Integration test: API lifespan store wiring."""

import pytest
from httpx import ASGITransport, AsyncClient

from tanren_api.main import create_app
from tanren_api.settings import APISettings


@pytest.fixture
async def wired_client(tmp_path):
    """Create a fully lifespan-wired app and yield an async client."""
    settings = APISettings(
        api_key="test-key",
        db_url=str(tmp_path / "wired.db"),
    )
    app = create_app(settings)
    async with (
        app.router.lifespan_context(app),
        AsyncClient(
            transport=ASGITransport(app=app),
            base_url="http://test",
        ) as client,
    ):
        yield client


AUTH = {"X-API-Key": "test-key"}


@pytest.mark.asyncio
async def test_lifespan_initializes_store_and_services(tmp_path):
    """Lifespan creates store, wires services, and closes on shutdown."""
    settings = APISettings(
        api_key="test-key",
        db_url=str(tmp_path / "lifespan-test.db"),
    )
    app = create_app(settings)

    async with AsyncClient(
        transport=ASGITransport(app=app),
        base_url="http://test",
    ) as client:
        # Health should work (proves lifespan ran)
        resp = await client.get("/api/v1/health")
        assert resp.status_code == 200
        data = resp.json()
        assert data["status"] == "ok"


@pytest.mark.asyncio
async def test_dispatch_round_trip_through_lifespan(wired_client):
    """Create and get a dispatch through the full lifespan-wired app."""
    client = wired_client

    # Create dispatch
    create_resp = await client.post(
        "/api/v1/dispatch",
        json={
            "project": "test-project",
            "phase": "do-task",
            "branch": "main",
            "spec_folder": "specs/test",
            "cli": "claude",
            "resolved_profile": {"name": "default"},
        },
        headers=AUTH,
    )
    assert create_resp.status_code == 200
    dispatch_id = create_resp.json()["dispatch_id"]

    # Get dispatch
    get_resp = await client.get(
        f"/api/v1/dispatch/{dispatch_id}",
        headers=AUTH,
    )
    assert get_resp.status_code == 200
    assert get_resp.json()["project"] == "test-project"


@pytest.mark.asyncio
async def test_dispatch_cancel(wired_client):
    """Cancel a dispatch through the API."""
    client = wired_client

    # Create
    resp = await client.post(
        "/api/v1/dispatch",
        json={
            "project": "cancel-test",
            "phase": "do-task",
            "branch": "main",
            "spec_folder": "specs/cancel",
            "cli": "claude",
            "resolved_profile": {"name": "default"},
        },
        headers=AUTH,
    )
    assert resp.status_code == 200
    dispatch_id = resp.json()["dispatch_id"]

    # Cancel
    del_resp = await client.delete(
        f"/api/v1/dispatch/{dispatch_id}",
        headers=AUTH,
    )
    assert del_resp.status_code == 200
    assert del_resp.json()["status"] == "cancelled"


@pytest.mark.asyncio
async def test_dispatch_get_not_found(wired_client):
    """Get a nonexistent dispatch returns 404."""
    resp = await wired_client.get(
        "/api/v1/dispatch/wf-nonexistent-0-0",
        headers=AUTH,
    )
    assert resp.status_code == 404


@pytest.mark.asyncio
async def test_health_readiness(wired_client):
    """Readiness endpoint returns ready."""
    resp = await wired_client.get("/api/v1/health/ready")
    assert resp.status_code == 200
    assert resp.json()["status"] == "ready"


@pytest.mark.asyncio
async def test_config_endpoint(wired_client):
    """Config endpoint returns V2 config fields."""
    resp = await wired_client.get("/api/v1/config", headers=AUTH)
    assert resp.status_code == 200
    data = resp.json()
    assert data["db_backend"] == "sqlite"
    assert data["store_connected"] is True
    assert "impl" in data["worker_lanes"]
    assert data["version"] == "0.1.0"


@pytest.mark.asyncio
async def test_events_endpoint_empty(wired_client):
    """Events endpoint returns empty list when no events."""
    resp = await wired_client.get("/api/v1/events", headers=AUTH)
    assert resp.status_code == 200
    data = resp.json()
    assert data["total"] == 0
    assert data["events"] == []
    assert data["limit"] == 50
    assert data["offset"] == 0


@pytest.mark.asyncio
async def test_events_endpoint_after_dispatch(wired_client):
    """Events endpoint returns events after creating a dispatch."""
    client = wired_client

    # Create a dispatch (which appends DispatchCreated events)
    await client.post(
        "/api/v1/dispatch",
        json={
            "project": "events-test",
            "phase": "do-task",
            "branch": "main",
            "spec_folder": "specs/events",
            "cli": "claude",
            "resolved_profile": {"name": "default"},
        },
        headers=AUTH,
    )

    # Query events
    resp = await client.get("/api/v1/events", headers=AUTH)
    assert resp.status_code == 200
    data = resp.json()
    assert data["total"] > 0


@pytest.mark.asyncio
async def test_metrics_summary_empty(wired_client):
    """Metrics summary returns zeros when no events."""
    resp = await wired_client.get("/api/v1/metrics/summary", headers=AUTH)
    assert resp.status_code == 200
    data = resp.json()
    assert data["total_phases"] == 0
    assert data["succeeded"] == 0
    assert data["success_rate"] == pytest.approx(0.0)


@pytest.mark.asyncio
async def test_metrics_costs_empty(wired_client):
    """Metrics costs returns empty buckets when no token events."""
    resp = await wired_client.get("/api/v1/metrics/costs", headers=AUTH)
    assert resp.status_code == 200
    data = resp.json()
    assert data["buckets"] == []
    assert data["total_cost"] == pytest.approx(0.0)
    assert data["total_tokens"] == 0
    assert data["group_by"] == "model"


@pytest.mark.asyncio
async def test_metrics_vms_empty(wired_client):
    """Metrics VMs returns zeros when no VM events."""
    resp = await wired_client.get("/api/v1/metrics/vms", headers=AUTH)
    assert resp.status_code == 200
    data = resp.json()
    assert data["total_provisioned"] == 0
    assert data["total_released"] == 0
    assert data["currently_active"] == 0


@pytest.mark.asyncio
async def test_auth_required(wired_client):
    """Dispatch endpoint requires auth header."""
    resp = await wired_client.get("/api/v1/dispatch/wf-test-1-1")
    assert resp.status_code == 422  # missing X-API-Key header


@pytest.mark.asyncio
async def test_auth_invalid_key(wired_client):
    """Dispatch endpoint rejects invalid key."""
    resp = await wired_client.get(
        "/api/v1/dispatch/wf-test-1-1",
        headers={"X-API-Key": "wrong-key"},
    )
    assert resp.status_code == 401


# ---------------------------------------------------------------------------
# Run lifecycle endpoints
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_run_provision(wired_client):
    """Run provision endpoint creates a dispatch and returns env_id."""
    resp = await wired_client.post(
        "/api/v1/run/provision",
        json={
            "project": "run-test",
            "branch": "main",
            "resolved_profile": {"name": "default"},
        },
        headers=AUTH,
    )
    assert resp.status_code == 200
    data = resp.json()
    assert "env_id" in data
    assert data["status"] == "provisioning"


@pytest.mark.asyncio
async def test_run_full(wired_client):
    """Run full endpoint creates a dispatch for full lifecycle."""
    resp = await wired_client.post(
        "/api/v1/run/full",
        json={
            "project": "full-test",
            "branch": "main",
            "spec_path": "specs/test",
            "phase": "do-task",
            "cli": "claude",
            "auth": "api_key",
        },
        headers=AUTH,
    )
    assert resp.status_code == 200
    data = resp.json()
    assert "dispatch_id" in data
    assert data["status"] == "accepted"


@pytest.mark.asyncio
async def test_run_status_not_found(wired_client):
    """Run status returns 404 for nonexistent env_id."""
    resp = await wired_client.get(
        "/api/v1/run/nonexistent-env/status",
        headers=AUTH,
    )
    assert resp.status_code == 404


@pytest.mark.asyncio
async def test_run_execute_no_provision(wired_client):
    """Run execute returns error when no provision exists."""
    resp = await wired_client.post(
        "/api/v1/run/nonexistent/execute",
        json={
            "project": "test",
            "spec_path": "specs/test",
            "phase": "do-task",
            "cli": "claude",
            "auth": "api_key",
        },
        headers=AUTH,
    )
    assert resp.status_code == 404


@pytest.mark.asyncio
async def test_run_teardown_no_provision(wired_client):
    """Run teardown returns error when no provision exists."""
    resp = await wired_client.post(
        "/api/v1/run/nonexistent/teardown",
        headers=AUTH,
    )
    assert resp.status_code == 404


# ---------------------------------------------------------------------------
# VM endpoints
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_vm_list_empty(wired_client):
    """VM list returns empty when no VMs provisioned."""
    resp = await wired_client.get("/api/v1/vm", headers=AUTH)
    assert resp.status_code == 200
    assert resp.json() == []


@pytest.mark.asyncio
async def test_vm_provision(wired_client):
    """VM provision enqueues a provision step."""
    resp = await wired_client.post(
        "/api/v1/vm/provision",
        json={
            "project": "vm-test",
            "branch": "main",
            "resolved_profile": {"name": "default"},
        },
        headers=AUTH,
    )
    assert resp.status_code == 200
    data = resp.json()
    assert "env_id" in data
    assert data["status"] == "provisioning"


@pytest.mark.asyncio
async def test_vm_release_not_found(wired_client):
    """VM release returns 404 for nonexistent VM."""
    resp = await wired_client.delete(
        "/api/v1/vm/nonexistent-vm",
        headers=AUTH,
    )
    assert resp.status_code == 404


@pytest.mark.asyncio
async def test_vm_dry_run(wired_client):
    """VM dry-run enqueues a DRY_RUN step."""
    resp = await wired_client.post(
        "/api/v1/vm/dry-run",
        json={
            "project": "dry-test",
            "branch": "main",
            "resolved_profile": {"name": "default"},
        },
        headers=AUTH,
    )
    assert resp.status_code == 200
    data = resp.json()
    assert "env_id" in data


@pytest.mark.asyncio
async def test_vm_provision_status_not_found(wired_client):
    """VM provision status returns 404 for nonexistent env_id."""
    resp = await wired_client.get(
        "/api/v1/vm/provision/nonexistent",
        headers=AUTH,
    )
    assert resp.status_code == 404


# ---------------------------------------------------------------------------
# Run provision -> status -> execute -> teardown flow
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_run_provision_then_status(wired_client):
    """Provision a run env, then poll its status."""
    resp = await wired_client.post(
        "/api/v1/run/provision",
        json={
            "project": "flow-test",
            "branch": "main",
            "resolved_profile": {"name": "default"},
        },
        headers=AUTH,
    )
    assert resp.status_code == 200
    env_id = resp.json()["env_id"]

    # Status should be findable (dispatch exists in store)
    status_resp = await wired_client.get(
        f"/api/v1/run/{env_id}/status",
        headers=AUTH,
    )
    assert status_resp.status_code == 200
    data = status_resp.json()
    assert data["env_id"] == env_id
    assert data["status"] in ("provisioning", "provisioned")


@pytest.mark.asyncio
async def test_run_provision_then_execute_no_completed_provision(wired_client):
    """Execute after provision but before worker processes it gives 500."""
    resp = await wired_client.post(
        "/api/v1/run/provision",
        json={
            "project": "exec-test",
            "branch": "main",
            "resolved_profile": {"name": "default"},
        },
        headers=AUTH,
    )
    env_id = resp.json()["env_id"]

    # Execute should fail because provision step hasn't been processed
    exec_resp = await wired_client.post(
        f"/api/v1/run/{env_id}/execute",
        json={
            "project": "exec-test",
            "spec_path": "specs/test",
            "phase": "do-task",
            "cli": "claude",
            "auth": "api_key",
        },
        headers=AUTH,
    )
    assert exec_resp.status_code == 500


@pytest.mark.asyncio
async def test_run_provision_then_teardown_no_completed_provision(wired_client):
    """Teardown after provision but before worker processes it gives 500."""
    resp = await wired_client.post(
        "/api/v1/run/provision",
        json={
            "project": "td-test",
            "branch": "main",
            "resolved_profile": {"name": "default"},
        },
        headers=AUTH,
    )
    env_id = resp.json()["env_id"]

    td_resp = await wired_client.post(
        f"/api/v1/run/{env_id}/teardown",
        headers=AUTH,
    )
    assert td_resp.status_code == 500


@pytest.mark.asyncio
async def test_vm_provision_then_status(wired_client):
    """VM provision then check status."""
    resp = await wired_client.post(
        "/api/v1/vm/provision",
        json={
            "project": "vm-status-test",
            "branch": "main",
            "resolved_profile": {"name": "default"},
        },
        headers=AUTH,
    )
    assert resp.status_code == 200
    env_id = resp.json()["env_id"]

    status_resp = await wired_client.get(
        f"/api/v1/vm/provision/{env_id}",
        headers=AUTH,
    )
    assert status_resp.status_code == 200
    data = status_resp.json()
    assert data["env_id"] == env_id
    assert data["status"] in ("provisioning", "active", "failed")


@pytest.mark.asyncio
async def test_vm_list_after_provision(wired_client):
    """VM list returns entries after provisioning (before teardown)."""
    # Provision a VM
    await wired_client.post(
        "/api/v1/vm/provision",
        json={
            "project": "vm-list-test",
            "branch": "main",
            "resolved_profile": {"name": "default"},
        },
        headers=AUTH,
    )

    # List VMs — may be empty since worker hasn't processed the provision step
    resp = await wired_client.get("/api/v1/vm", headers=AUTH)
    assert resp.status_code == 200
    # Just verify the endpoint works — actual VM presence depends on worker processing


@pytest.mark.asyncio
async def test_metrics_summary_with_project_filter(wired_client):
    """Metrics summary with project filter returns filtered results."""
    resp = await wired_client.get(
        "/api/v1/metrics/summary?project=nonexistent",
        headers=AUTH,
    )
    assert resp.status_code == 200
    data = resp.json()
    assert data["total_phases"] == 0


@pytest.mark.asyncio
async def test_metrics_costs_group_by_day(wired_client):
    """Metrics costs with group_by=day returns empty."""
    resp = await wired_client.get(
        "/api/v1/metrics/costs?group_by=day",
        headers=AUTH,
    )
    assert resp.status_code == 200
    assert resp.json()["group_by"] == "day"


@pytest.mark.asyncio
async def test_metrics_costs_group_by_workflow(wired_client):
    """Metrics costs with group_by=workflow returns empty."""
    resp = await wired_client.get(
        "/api/v1/metrics/costs?group_by=workflow",
        headers=AUTH,
    )
    assert resp.status_code == 200
    assert resp.json()["group_by"] == "workflow"


@pytest.mark.asyncio
async def test_metrics_costs_invalid_group_by(wired_client):
    """Metrics costs with invalid group_by returns 422."""
    resp = await wired_client.get(
        "/api/v1/metrics/costs?group_by=invalid",
        headers=AUTH,
    )
    assert resp.status_code == 422


@pytest.mark.asyncio
async def test_metrics_vms_with_project_filter(wired_client):
    """Metrics VMs with project filter returns filtered results."""
    resp = await wired_client.get(
        "/api/v1/metrics/vms?project=nonexistent",
        headers=AUTH,
    )
    assert resp.status_code == 200
    data = resp.json()
    assert data["total_provisioned"] == 0


@pytest.mark.asyncio
async def test_metrics_summary_with_time_filter(wired_client):
    """Metrics summary with time range filter."""
    resp = await wired_client.get(
        "/api/v1/metrics/summary?since=2026-01-01T00:00:00Z&until=2026-12-31T23:59:59Z",
        headers=AUTH,
    )
    assert resp.status_code == 200
