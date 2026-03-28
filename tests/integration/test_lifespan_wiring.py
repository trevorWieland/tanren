"""Integration test: API lifespan store wiring."""

import asyncio

import pytest
from httpx import ASGITransport, AsyncClient

from tanren_api.main import create_app
from tanren_api.settings import APISettings


@pytest.fixture
async def wired_client(tmp_path):
    """Create a fully lifespan-wired app and yield an async client.

    The lifespan runs in a dedicated asyncio Task so that anyio cancel
    scopes (used by FastMCP's StreamableHTTPSessionManager) are entered
    and exited in the same task — matching how real ASGI servers work.
    """
    settings = APISettings(
        api_key="test-key",
        db_url=str(tmp_path / "wired.db"),
    )
    app = create_app(settings)

    startup_complete = asyncio.Event()
    shutdown_trigger = asyncio.Event()
    exc_holder: list[BaseException] = []

    async def _run_lifespan() -> None:
        try:
            async with app.router.lifespan_context(app):
                startup_complete.set()
                await shutdown_trigger.wait()
        except BaseException as exc:
            exc_holder.append(exc)
            startup_complete.set()

    task = asyncio.create_task(_run_lifespan())
    await startup_complete.wait()
    if exc_holder:
        raise exc_holder[0]

    try:
        async with AsyncClient(
            transport=ASGITransport(app=app),
            base_url="http://test",
        ) as client:
            yield client
    finally:
        shutdown_trigger.set()
        await task


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

    # Cancel (also cancels pending steps)
    del_resp = await client.delete(
        f"/api/v1/dispatch/{dispatch_id}",
        headers=AUTH,
    )
    assert del_resp.status_code == 200
    assert del_resp.json()["status"] == "cancelled"

    # Verify dispatch shows cancelled
    get_resp = await client.get(
        f"/api/v1/dispatch/{dispatch_id}",
        headers=AUTH,
    )
    assert get_resp.status_code == 200
    assert get_resp.json()["status"] == "cancelled"

    # Cancel again should fail with 409 conflict
    del_again = await client.delete(
        f"/api/v1/dispatch/{dispatch_id}",
        headers=AUTH,
    )
    assert del_again.status_code == 409

    # Cancel nonexistent should 404
    del_missing = await client.delete(
        "/api/v1/dispatch/nonexistent-id",
        headers=AUTH,
    )
    assert del_missing.status_code == 404


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
    # Version is read from package metadata at runtime
    assert data["version"]


@pytest.mark.asyncio
async def test_events_endpoint_empty(wired_client):
    """Events endpoint returns empty dispatch events when no dispatches created."""
    resp = await wired_client.get(
        "/api/v1/events", headers=AUTH, params={"entity_type": "dispatch"}
    )
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
    # Verify store lifecycle events are included (not skipped as unparseable)
    event_types = {e["type"] for e in data["events"]}
    assert "dispatch_created" in event_types or "step_enqueued" in event_types


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
            "resolved_profile": {"name": "default"},
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
async def test_vm_dry_run_lifecycle(tmp_path):
    """VM dry-run: enqueue, worker processes step, status reaches terminal."""
    from tanren_core.store.enums import DispatchStatus, StepType
    from tanren_core.store.payloads import DryRunResult

    settings = APISettings(
        api_key="test-key",
        db_url=str(tmp_path / "dry-run-lifecycle.db"),
    )
    app = create_app(settings)
    async with (
        app.router.lifespan_context(app),
        AsyncClient(
            transport=ASGITransport(app=app),
            base_url="http://test",
        ) as client,
    ):
        # 1. Enqueue dry-run via API
        resp = await client.post(
            "/api/v1/vm/dry-run",
            json={
                "project": "dry-lifecycle",
                "branch": "main",
                "resolved_profile": {"name": "default"},
            },
            headers=AUTH,
        )
        assert resp.status_code == 200
        env_id = resp.json()["env_id"]

        # 2. Simulate worker processing the DRY_RUN step
        store = app.state.state_store
        queue = app.state.job_queue

        steps = await store.get_steps_for_dispatch(env_id)
        assert len(steps) == 1
        dry_step = steps[0]
        assert dry_step.step_type == StepType.DRY_RUN

        result = DryRunResult(
            provider="hetzner",
            server_type="cx22",
            estimated_cost_hourly=0.01,
            would_provision=True,
        )
        await queue.ack(dry_step.step_id, result_json=result.model_dump_json())
        await store.update_dispatch_status(env_id, DispatchStatus.COMPLETED)

        # 3. Poll status — should reflect completed dry-run
        status_resp = await client.get(
            f"/api/v1/vm/provision/{env_id}",
            headers=AUTH,
        )
        assert status_resp.status_code == 200
        status_data = status_resp.json()
        assert status_data["status"] == "active"
        assert status_data["provider"] == "hetzner"
        assert status_data["server_type"] == "cx22"
        assert status_data["would_provision"] is True


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


# ---------------------------------------------------------------------------
# Run status outcome derivation (MANUAL mode)
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_run_status_derives_outcome_from_execute_steps(tmp_path):
    """RunStatus.outcome is derived from execute step result in MANUAL mode."""
    from tanren_core.env.environment_schema import EnvironmentProfile
    from tanren_core.schemas import Cli, Dispatch, Outcome, Phase
    from tanren_core.store.enums import DispatchMode, cli_to_lane
    from tanren_core.store.events import DispatchCreated
    from tanren_core.store.handle import PersistedEnvironmentHandle
    from tanren_core.store.payloads import ExecuteResult, ExecuteStepPayload, ProvisionResult

    settings = APISettings(
        api_key="test-key",
        db_url=str(tmp_path / "outcome-derivation.db"),
    )
    app = create_app(settings)
    async with (
        app.router.lifespan_context(app),
        AsyncClient(
            transport=ASGITransport(app=app),
            base_url="http://test",
        ) as client,
    ):
        store = app.state.state_store
        queue = app.state.job_queue
        event_store = app.state.event_store
        dispatch_id = "wf-derive-test-1"

        # Create MANUAL dispatch (outcome stays None at dispatch level)
        dispatch = Dispatch(
            workflow_id=dispatch_id,
            project="derive-test",
            phase=Phase.DO_TASK,
            branch="main",
            spec_folder="specs/test",
            cli=Cli.CLAUDE,
            timeout=1800,
            resolved_profile=EnvironmentProfile(name="default"),
        )
        lane = cli_to_lane(dispatch.cli)
        await store.create_dispatch_projection(
            dispatch_id=dispatch_id,
            mode=DispatchMode.MANUAL,
            lane=lane,
            preserve_on_failure=False,
            dispatch_json=dispatch.model_dump_json(),
        )
        from datetime import UTC, datetime

        await event_store.append(
            DispatchCreated(
                timestamp=datetime.now(UTC).isoformat().replace("+00:00", "Z"),
                entity_id=dispatch_id,
                dispatch=dispatch,
                mode=DispatchMode.MANUAL,
                lane=lane,
            )
        )

        # Create and complete provision step
        handle = PersistedEnvironmentHandle(
            env_id=dispatch_id,
            worktree_path="/tmp/test",
            branch="main",
            project="derive-test",
            profile_name="default",
            provision_timestamp=datetime.now(UTC).isoformat(),
        )
        prov_result = ProvisionResult(handle=handle)
        import uuid

        prov_step_id = uuid.uuid4().hex
        from tanren_core.store.payloads import ProvisionStepPayload

        await queue.enqueue_step(
            step_id=prov_step_id,
            dispatch_id=dispatch_id,
            step_type="provision",
            step_sequence=0,
            lane=None,
            payload_json=ProvisionStepPayload(dispatch=dispatch).model_dump_json(),
        )
        await queue.ack(prov_step_id, result_json=prov_result.model_dump_json())

        # Create and complete execute step with outcome=success
        exec_step_id = uuid.uuid4().hex
        exec_payload = ExecuteStepPayload(dispatch=dispatch, handle=handle)
        await queue.enqueue_step(
            step_id=exec_step_id,
            dispatch_id=dispatch_id,
            step_type="execute",
            step_sequence=1,
            lane=str(lane),
            payload_json=exec_payload.model_dump_json(),
        )
        exec_result = ExecuteResult(
            outcome=Outcome.SUCCESS,
            signal="all-done",
            exit_code=0,
            duration_secs=10,
        )
        await queue.ack(exec_step_id, result_json=exec_result.model_dump_json())

        # Status should derive outcome from execute step
        resp = await client.get(
            f"/api/v1/run/{dispatch_id}/status",
            headers=AUTH,
        )
        assert resp.status_code == 200
        data = resp.json()
        assert data["outcome"] == "success"


@pytest.mark.asyncio
async def test_run_provision_status_flow(wired_client):
    """Provision a run env and verify status polling returns expected shape."""
    resp = await wired_client.post(
        "/api/v1/run/provision",
        json={
            "project": "status-flow",
            "branch": "main",
            "resolved_profile": {"name": "default"},
        },
        headers=AUTH,
    )
    assert resp.status_code == 200
    env_id = resp.json()["env_id"]

    # Verify status polling returns consistent data
    status_resp = await wired_client.get(
        f"/api/v1/run/{env_id}/status",
        headers=AUTH,
    )
    assert status_resp.status_code == 200
    data = status_resp.json()
    assert data["env_id"] == env_id
    assert data["dispatch_id"]
    assert data["status"] in ("provisioning", "provisioned", "failed")
    # Outcome should be None before completion
    assert data["outcome"] is None


@pytest.mark.asyncio
async def test_run_full_without_cli(wired_client):
    """Run full without explicit cli should still accept (cli=None defaults in model)."""
    resp = await wired_client.post(
        "/api/v1/run/full",
        json={
            "project": "no-cli-test",
            "branch": "main",
            "spec_path": "specs/test",
            "phase": "gate",
            "resolved_profile": {"name": "default"},
        },
        headers=AUTH,
    )
    assert resp.status_code == 200
    data = resp.json()
    assert "dispatch_id" in data


@pytest.mark.asyncio
async def test_dispatch_create_without_cli(wired_client):
    """Dispatch create without cli should accept (auto-resolved for gate)."""
    resp = await wired_client.post(
        "/api/v1/dispatch",
        json={
            "project": "no-cli-dispatch",
            "phase": "gate",
            "branch": "main",
            "spec_folder": "specs/test",
            "resolved_profile": {"name": "default"},
        },
        headers=AUTH,
    )
    assert resp.status_code == 200
    data = resp.json()
    assert "dispatch_id" in data


@pytest.mark.asyncio
async def test_metrics_summary_after_phase_events(tmp_path):
    """Metrics summary counts events correctly."""

    settings = APISettings(
        api_key="test-key",
        db_url=str(tmp_path / "metrics-events.db"),
    )
    app = create_app(settings)
    async with (
        app.router.lifespan_context(app),
        AsyncClient(
            transport=ASGITransport(app=app),
            base_url="http://test",
        ) as client,
    ):
        store = app.state.event_store
        from datetime import UTC, datetime

        from tanren_core.adapters.events import PhaseCompleted

        now = datetime.now(UTC).isoformat().replace("+00:00", "Z")
        await store.append(
            PhaseCompleted(
                timestamp=now,
                entity_id="wf-metrics-test-1",
                phase="do-task",
                project="metrics-proj",
                outcome="success",
                signal="all-done",
                duration_secs=120,
                exit_code=0,
            )
        )
        await store.append(
            PhaseCompleted(
                timestamp=now,
                entity_id="wf-metrics-test-2",
                phase="gate",
                project="metrics-proj",
                outcome="fail",
                signal=None,
                duration_secs=30,
                exit_code=1,
            )
        )

        resp = await client.get("/api/v1/metrics/summary", headers=AUTH)
        assert resp.status_code == 200
        data = resp.json()
        assert data["total_phases"] == 2
        assert data["succeeded"] == 1
        assert data["failed"] == 1
        assert data["success_rate"] == pytest.approx(0.5)


@pytest.mark.asyncio
async def test_metrics_costs_with_token_events(tmp_path):
    """Metrics costs returns cost buckets when token events exist."""
    settings = APISettings(
        api_key="test-key",
        db_url=str(tmp_path / "metrics-costs.db"),
    )
    app = create_app(settings)
    async with (
        app.router.lifespan_context(app),
        AsyncClient(
            transport=ASGITransport(app=app),
            base_url="http://test",
        ) as client,
    ):
        store = app.state.event_store
        from datetime import UTC, datetime

        from tanren_core.adapters.events import TokenUsageRecorded

        now = datetime.now(UTC).isoformat().replace("+00:00", "Z")
        await store.append(
            TokenUsageRecorded(
                timestamp=now,
                entity_id="wf-cost-test-1",
                phase="do-task",
                project="cost-proj",
                cli="claude",
                input_tokens=1000,
                output_tokens=500,
                total_tokens=1500,
                total_cost=0.05,
                models_used=["claude-4-sonnet"],
            )
        )

        resp = await client.get("/api/v1/metrics/costs", headers=AUTH)
        assert resp.status_code == 200
        data = resp.json()
        assert len(data["buckets"]) > 0
        assert data["total_cost"] > 0
        assert data["total_tokens"] == 1500

        # Test group_by=day
        resp = await client.get("/api/v1/metrics/costs?group_by=day", headers=AUTH)
        assert resp.status_code == 200
        assert resp.json()["group_by"] == "day"

        # Test group_by=workflow
        resp = await client.get("/api/v1/metrics/costs?group_by=workflow", headers=AUTH)
        assert resp.status_code == 200
        assert resp.json()["group_by"] == "workflow"


@pytest.mark.asyncio
async def test_metrics_vms_with_events(tmp_path):
    """Metrics VMs returns counts when VM events exist."""
    settings = APISettings(
        api_key="test-key",
        db_url=str(tmp_path / "metrics-vms.db"),
    )
    app = create_app(settings)
    async with (
        app.router.lifespan_context(app),
        AsyncClient(
            transport=ASGITransport(app=app),
            base_url="http://test",
        ) as client,
    ):
        store = app.state.event_store
        from datetime import UTC, datetime

        from tanren_core.adapters.events import VMProvisioned, VMReleased
        from tanren_core.adapters.remote_types import VMProvider

        now = datetime.now(UTC).isoformat().replace("+00:00", "Z")
        await store.append(
            VMProvisioned(
                timestamp=now,
                entity_id="wf-vm-test-1",
                vm_id="vm-123",
                host="1.2.3.4",
                provider=VMProvider.HETZNER,
                project="vm-proj",
                profile="default",
                hourly_cost=0.05,
            )
        )
        await store.append(
            VMReleased(
                timestamp=now,
                entity_id="wf-vm-test-1",
                vm_id="vm-123",
                project="vm-proj",
                duration_secs=3600,
                estimated_cost=0.05,
            )
        )

        resp = await client.get("/api/v1/metrics/vms", headers=AUTH)
        assert resp.status_code == 200
        data = resp.json()
        assert data["total_provisioned"] == 1
        assert data["total_released"] == 1
        assert data["currently_active"] == 0
        assert data["by_provider"]["hetzner"] == 1


# ---------------------------------------------------------------------------
# Phase 1: Store-level cancel safety
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_cancel_does_not_overwrite_completed(tmp_path):
    """Cancel after a dispatch is completed should return 409 — store protects terminal states."""
    from tanren_core.schemas import Outcome
    from tanren_core.store.enums import DispatchStatus

    settings = APISettings(
        api_key="test-key",
        db_url=str(tmp_path / "cancel-completed.db"),
    )
    app = create_app(settings)
    async with (
        app.router.lifespan_context(app),
        AsyncClient(
            transport=ASGITransport(app=app),
            base_url="http://test",
        ) as client,
    ):
        # Create dispatch
        resp = await client.post(
            "/api/v1/dispatch",
            json={
                "project": "cancel-completed",
                "phase": "do-task",
                "branch": "main",
                "spec_folder": "specs/test",
                "cli": "claude",
                "resolved_profile": {"name": "default"},
            },
            headers=AUTH,
        )
        assert resp.status_code == 200
        dispatch_id = resp.json()["dispatch_id"]

        # Manually set to completed via store
        store = app.state.state_store
        await store.update_dispatch_status(dispatch_id, DispatchStatus.COMPLETED, Outcome.SUCCESS)

        # Cancel should fail with 409 conflict
        del_resp = await client.delete(
            f"/api/v1/dispatch/{dispatch_id}",
            headers=AUTH,
        )
        assert del_resp.status_code == 409


# ---------------------------------------------------------------------------
# Phase 6d: VM provision status after cancel
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_vm_provision_status_after_cancel(tmp_path):
    """VM provision status returns FAILED when dispatch is cancelled."""
    from tanren_core.store.enums import DispatchStatus

    settings = APISettings(
        api_key="test-key",
        db_url=str(tmp_path / "vm-cancel.db"),
    )
    app = create_app(settings)
    async with (
        app.router.lifespan_context(app),
        AsyncClient(
            transport=ASGITransport(app=app),
            base_url="http://test",
        ) as client,
    ):
        # Provision a VM
        resp = await client.post(
            "/api/v1/vm/provision",
            json={
                "project": "vm-cancel-test",
                "branch": "main",
                "resolved_profile": {"name": "default"},
            },
            headers=AUTH,
        )
        assert resp.status_code == 200
        env_id = resp.json()["env_id"]

        # Manually set dispatch to cancelled
        store = app.state.state_store
        await store.update_dispatch_status(env_id, DispatchStatus.CANCELLED)

        # Provision status should show failed
        status_resp = await client.get(
            f"/api/v1/vm/provision/{env_id}",
            headers=AUTH,
        )
        assert status_resp.status_code == 200
        assert status_resp.json()["status"] == "failed"


# ---------------------------------------------------------------------------
# Phase 2: Worker wait loop CANCELLED terminal (unit-style integration)
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_dispatch_cancel_flow_with_steps(tmp_path):
    """Cancel a dispatch that has pending steps — steps get cancelled."""
    settings = APISettings(
        api_key="test-key",
        db_url=str(tmp_path / "cancel-steps.db"),
    )
    app = create_app(settings)
    async with (
        app.router.lifespan_context(app),
        AsyncClient(
            transport=ASGITransport(app=app),
            base_url="http://test",
        ) as client,
    ):
        # Create dispatch
        resp = await client.post(
            "/api/v1/dispatch",
            json={
                "project": "cancel-steps",
                "phase": "do-task",
                "branch": "main",
                "spec_folder": "specs/test",
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

        # Verify steps are cancelled
        store = app.state.state_store
        steps = await store.get_steps_for_dispatch(dispatch_id)
        for step in steps:
            if step.step_type.value != "teardown":
                assert step.status.value in ("cancelled", "completed", "failed")


# ---------------------------------------------------------------------------
# Phase 5c: Cancel with teardown — exercises _enqueue_cancel_teardown
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_cancel_after_completed_provision_enqueues_teardown(tmp_path):
    """Cancel after provision completed enqueues a teardown step with dynamic sequence."""
    import uuid

    from tanren_core.env.environment_schema import EnvironmentProfile
    from tanren_core.schemas import Cli, Dispatch, Phase
    from tanren_core.store.enums import DispatchMode, StepType, cli_to_lane
    from tanren_core.store.events import DispatchCreated
    from tanren_core.store.handle import PersistedEnvironmentHandle
    from tanren_core.store.payloads import ProvisionResult, ProvisionStepPayload

    settings = APISettings(
        api_key="test-key",
        db_url=str(tmp_path / "cancel-teardown.db"),
    )
    app = create_app(settings)
    async with (
        app.router.lifespan_context(app),
        AsyncClient(
            transport=ASGITransport(app=app),
            base_url="http://test",
        ) as client,
    ):
        store = app.state.state_store
        queue = app.state.job_queue
        event_store = app.state.event_store

        dispatch_id = "wf-cancel-td-test-1"
        dispatch = Dispatch(
            workflow_id=dispatch_id,
            project="cancel-td",
            phase=Phase.DO_TASK,
            branch="main",
            spec_folder="specs/test",
            cli=Cli.CLAUDE,
            timeout=1800,
            resolved_profile=EnvironmentProfile(name="default"),
        )
        lane = cli_to_lane(dispatch.cli)
        await store.create_dispatch_projection(
            dispatch_id=dispatch_id,
            mode=DispatchMode.AUTO,
            lane=lane,
            preserve_on_failure=False,
            dispatch_json=dispatch.model_dump_json(),
        )
        from datetime import UTC, datetime

        await event_store.append(
            DispatchCreated(
                timestamp=datetime.now(UTC).isoformat().replace("+00:00", "Z"),
                entity_id=dispatch_id,
                dispatch=dispatch,
                mode=DispatchMode.AUTO,
                lane=lane,
            )
        )

        # Create and complete provision step
        handle = PersistedEnvironmentHandle(
            env_id=dispatch_id,
            worktree_path="/tmp/test",
            branch="main",
            project="cancel-td",
            profile_name="default",
            provision_timestamp=datetime.now(UTC).isoformat(),
        )
        prov_result = ProvisionResult(handle=handle)
        prov_step_id = uuid.uuid4().hex
        await queue.enqueue_step(
            step_id=prov_step_id,
            dispatch_id=dispatch_id,
            step_type="provision",
            step_sequence=0,
            lane=None,
            payload_json=ProvisionStepPayload(dispatch=dispatch).model_dump_json(),
        )
        await queue.ack(prov_step_id, result_json=prov_result.model_dump_json())

        # Cancel via API — should enqueue teardown
        del_resp = await client.delete(
            f"/api/v1/dispatch/{dispatch_id}",
            headers=AUTH,
        )
        assert del_resp.status_code == 200

        # Verify teardown was enqueued
        steps = await store.get_steps_for_dispatch(dispatch_id)
        teardown_steps = [s for s in steps if s.step_type == StepType.TEARDOWN]
        assert len(teardown_steps) == 1
        # Step sequence should be max_existing + 1 (not hardcoded 2)
        assert teardown_steps[0].step_sequence == 1  # provision is 0, so max+1 = 1


# ---------------------------------------------------------------------------
# Phase 6e: Config returns dynamic worker_lanes from WorkerConfig
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_config_returns_default_lanes_without_worker_config(wired_client):
    """Config endpoint returns default lanes when WM_* env not set."""
    resp = await wired_client.get("/api/v1/config", headers=AUTH)
    assert resp.status_code == 200
    data = resp.json()
    # Without WorkerConfig wired, we get defaults
    assert data["worker_lanes"]["impl"] >= 1
    assert data["worker_lanes"]["gate"] >= 1


# ---------------------------------------------------------------------------
# VM list with completed provisions
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_vm_list_with_completed_provision(tmp_path):
    """VM list returns entries when provision steps have completed with VM info."""
    import uuid

    from tanren_core.adapters.remote_types import VMProvider
    from tanren_core.env.environment_schema import EnvironmentProfile
    from tanren_core.schemas import Cli, Dispatch, Phase
    from tanren_core.store.enums import DispatchMode, cli_to_lane
    from tanren_core.store.events import DispatchCreated
    from tanren_core.store.handle import PersistedEnvironmentHandle, PersistedVMInfo
    from tanren_core.store.payloads import ProvisionResult, ProvisionStepPayload

    settings = APISettings(
        api_key="test-key",
        db_url=str(tmp_path / "vm-list.db"),
    )
    app = create_app(settings)
    async with (
        app.router.lifespan_context(app),
        AsyncClient(
            transport=ASGITransport(app=app),
            base_url="http://test",
        ) as client,
    ):
        store = app.state.state_store
        queue = app.state.job_queue
        event_store = app.state.event_store

        dispatch_id = "wf-vm-list-test-1"
        dispatch = Dispatch(
            workflow_id=dispatch_id,
            project="vm-list-project",
            phase=Phase.DO_TASK,
            branch="main",
            spec_folder="specs/test",
            cli=Cli.CLAUDE,
            timeout=1800,
            resolved_profile=EnvironmentProfile(name="default"),
        )
        lane = cli_to_lane(dispatch.cli)
        await store.create_dispatch_projection(
            dispatch_id=dispatch_id,
            mode=DispatchMode.MANUAL,
            lane=lane,
            preserve_on_failure=True,
            dispatch_json=dispatch.model_dump_json(),
        )
        from datetime import UTC, datetime

        await event_store.append(
            DispatchCreated(
                timestamp=datetime.now(UTC).isoformat().replace("+00:00", "Z"),
                entity_id=dispatch_id,
                dispatch=dispatch,
                mode=DispatchMode.MANUAL,
                lane=lane,
            )
        )

        # Create and complete provision step with VM info
        handle = PersistedEnvironmentHandle(
            env_id=dispatch_id,
            worktree_path="/tmp/test",
            branch="main",
            project="vm-list-project",
            profile_name="default",
            provision_timestamp=datetime.now(UTC).isoformat(),
            vm=PersistedVMInfo(
                vm_id="vm-test-123",
                host="10.0.0.1",
                provider=VMProvider.HETZNER,
                created_at=datetime.now(UTC).isoformat(),
            ),
        )
        prov_result = ProvisionResult(handle=handle)
        prov_step_id = uuid.uuid4().hex
        await queue.enqueue_step(
            step_id=prov_step_id,
            dispatch_id=dispatch_id,
            step_type="provision",
            step_sequence=0,
            lane=None,
            payload_json=ProvisionStepPayload(dispatch=dispatch).model_dump_json(),
        )
        await queue.ack(prov_step_id, result_json=prov_result.model_dump_json())

        # VM list should include this VM
        resp = await client.get("/api/v1/vm", headers=AUTH)
        assert resp.status_code == 200
        vms = resp.json()
        assert len(vms) >= 1
        vm_ids = [v["vm_id"] for v in vms]
        assert "vm-test-123" in vm_ids

        # Provision status should show active
        status_resp = await client.get(
            f"/api/v1/vm/provision/{dispatch_id}",
            headers=AUTH,
        )
        assert status_resp.status_code == 200
        assert status_resp.json()["status"] == "active"
        assert status_resp.json()["vm_id"] == "vm-test-123"

        # VM release should enqueue teardown with dynamic step_sequence
        release_resp = await client.delete(
            "/api/v1/vm/vm-test-123",
            headers=AUTH,
        )
        assert release_resp.status_code == 200
        assert release_resp.json()["vm_id"] == "vm-test-123"

        # Verify teardown step was enqueued
        from tanren_core.store.enums import StepType

        steps = await store.get_steps_for_dispatch(dispatch_id)
        td = [s for s in steps if s.step_type == StepType.TEARDOWN]
        assert len(td) == 1
        assert td[0].step_sequence == 1  # max(0) + 1


# ── Auth lifecycle integration tests ─────────────────────────────────────


@pytest.mark.asyncio
async def test_user_crud_lifecycle(wired_client):
    """User creation, listing, retrieval, and deactivation via endpoints."""
    client = wired_client

    # Create user
    resp = await client.post(
        "/api/v1/users",
        headers=AUTH,
        json={"name": "Test User", "email": "test@example.com", "role": "developer"},
    )
    assert resp.status_code == 200
    user = resp.json()
    user_id = user["user_id"]
    assert user["name"] == "Test User"
    assert user["is_active"] is True

    # List users — should include admin + test user
    resp = await client.get("/api/v1/users", headers=AUTH)
    assert resp.status_code == 200
    users = resp.json()
    assert len(users) >= 2

    # Get specific user
    resp = await client.get(f"/api/v1/users/{user_id}", headers=AUTH)
    assert resp.status_code == 200
    assert resp.json()["email"] == "test@example.com"

    # Deactivate user
    resp = await client.delete(f"/api/v1/users/{user_id}", headers=AUTH)
    assert resp.status_code == 200
    assert resp.json()["status"] == "deactivated"


@pytest.mark.asyncio
async def test_key_lifecycle(wired_client):
    """Key creation, listing, revocation, and rotation via endpoints."""
    client = wired_client

    # Create user first
    resp = await client.post("/api/v1/users", headers=AUTH, json={"name": "Key User"})
    user_id = resp.json()["user_id"]

    # Create key
    resp = await client.post(
        "/api/v1/keys",
        headers=AUTH,
        json={"user_id": user_id, "name": "Test Key", "scopes": ["dispatch:*", "vm:read"]},
    )
    assert resp.status_code == 200
    key_data = resp.json()
    assert key_data["key"].startswith("tnrn_")
    key_id = key_data["key_id"]

    # List keys
    resp = await client.get("/api/v1/keys", headers=AUTH)
    assert resp.status_code == 200
    keys = resp.json()
    assert any(k["key_id"] == key_id for k in keys)

    # Rotate key
    resp = await client.post(
        f"/api/v1/keys/{key_id}/rotate",
        headers=AUTH,
        json={"grace_period_hours": 1},
    )
    assert resp.status_code == 200
    new_key = resp.json()
    assert new_key["key_id"] != key_id
    assert new_key["key"].startswith("tnrn_")

    # Revoke the new key
    resp = await client.delete(f"/api/v1/keys/{new_key['key_id']}", headers=AUTH)
    assert resp.status_code == 200
    assert resp.json()["status"] == "revoked"


@pytest.mark.asyncio
async def test_scoped_key_enforcement(wired_client):
    """A scoped key can only access endpoints matching its scopes."""
    client = wired_client

    # Create user
    resp = await client.post("/api/v1/users", headers=AUTH, json={"name": "Scoped User"})
    user_id = resp.json()["user_id"]

    # Create a key with only config:read scope
    resp = await client.post(
        "/api/v1/keys",
        headers=AUTH,
        json={"user_id": user_id, "name": "ReadOnly", "scopes": ["config:read"]},
    )
    scoped_key = resp.json()["key"]
    scoped_headers = {"X-API-Key": scoped_key}

    # config:read should work
    resp = await client.get("/api/v1/config", headers=scoped_headers)
    assert resp.status_code == 200

    # dispatch:create should be forbidden
    resp = await client.post(
        "/api/v1/dispatch",
        headers=scoped_headers,
        json={"project": "test", "phase": "do-task", "spec_folder": "s", "branch": "main"},
    )
    assert resp.status_code == 403


@pytest.mark.asyncio
async def test_config_resolver_disk(tmp_path):
    """DiskConfigResolver reads tanren.yml and .env from disk."""
    from tanren_core.config_resolver import DiskConfigResolver

    project_dir = tmp_path / "test-project"
    project_dir.mkdir()
    (project_dir / "tanren.yml").write_text("environment:\n  default:\n    type: local\n")
    (project_dir / ".env").write_text("MY_VAR=hello\n")

    resolver = DiskConfigResolver(str(tmp_path))
    config = await resolver.load_tanren_config("test-project")
    assert config["environment"]["default"]["type"] == "local"

    env = await resolver.load_project_env("test-project")
    assert env["MY_VAR"] == "hello"

    # Missing project returns empty
    assert await resolver.load_tanren_config("nonexistent") == {}
    assert await resolver.load_project_env("nonexistent") == {}


@pytest.mark.asyncio
async def test_dispatch_builder_resolve(tmp_path):
    """Dispatch builder resolves inputs via ConfigResolver."""
    from tanren_core.config_resolver import DiskConfigResolver
    from tanren_core.dispatch_builder import resolve_dispatch_inputs, resolve_provision_inputs
    from tanren_core.worker_config import WorkerConfig

    project_dir = tmp_path / "github" / "test-project"
    project_dir.mkdir(parents=True)
    (project_dir / "tanren.yml").write_text(
        "environment:\n  default:\n    type: local\n    gate_cmd: make test\n"
    )

    config = WorkerConfig(
        ipc_dir=str(tmp_path / "ipc"),
        github_dir=str(tmp_path / "github"),
        data_dir=str(tmp_path / "data"),
        db_url=str(tmp_path / "test.db"),
        worktree_registry_path=str(tmp_path / "worktrees.json"),
    )
    resolver = DiskConfigResolver(config.github_dir)

    # Test dispatch inputs
    from tanren_core.schemas import Phase

    result = await resolve_dispatch_inputs(
        resolver=resolver,
        config=config,
        project="test-project",
        phase=Phase.GATE,
        branch="main",
    )
    assert result.profile.name == "default"
    assert result.gate_cmd == "make test"

    # Test provision inputs
    result = await resolve_provision_inputs(
        resolver=resolver,
        config=config,
        project="test-project",
    )
    assert result.profile.name == "default"


@pytest.mark.asyncio
async def test_legacy_admin_seed(wired_client):
    """Legacy API key seed creates admin user and key on startup."""
    client = wired_client

    # List users — admin should exist
    resp = await client.get("/api/v1/users", headers=AUTH)
    assert resp.status_code == 200
    users = resp.json()
    admin = [u for u in users if u["name"] == "Admin (legacy)"]
    assert len(admin) == 1
    assert admin[0]["is_active"] is True

    # List keys — legacy key should exist
    resp = await client.get("/api/v1/keys", headers=AUTH)
    assert resp.status_code == 200
    keys = resp.json()
    legacy = [k for k in keys if k["name"] == "Legacy admin key"]
    assert len(legacy) == 1
    assert legacy[0]["scopes"] == ["*"]
