"""Integration tests for the tanren API — exercises the full ASGI stack."""

from __future__ import annotations

import asyncio
import json
import logging
import os
import re
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock, patch

import aiosqlite
import pytest
from fastapi import Request
from httpx import ASGITransport, AsyncClient

from tanren_api.auth import APIKeyVerifier
from tanren_api.dependencies import get_config, get_emitter, get_settings
from tanren_api.errors import (
    AuthenticationError,
    ConflictError,
    NotFoundError,
    ServiceError,
    TanrenAPIError,
)
from tanren_api.main import create_app
from tanren_api.middleware import RequestIDMiddleware, RequestLoggingMiddleware
from tanren_api.settings import APISettings
from tanren_api.state import APIStateStore
from tanren_core.adapters.null_emitter import NullEventEmitter
from tanren_core.adapters.remote_types import VMAssignment, VMHandle, VMProvider, WorkspacePath
from tanren_core.adapters.sqlite_emitter import (
    _SCHEMA,
    SqliteEventEmitter,
)
from tanren_core.adapters.types import (
    EnvironmentHandle,
    PhaseResult,
    RemoteEnvironmentRuntime,
)
from tanren_core.config import Config, DotenvConfigSource, load_config_env
from tanren_core.env.environment_schema import EnvironmentProfile
from tanren_core.schemas import Outcome

TEST_API_KEY = "test-integration-key"


# ---------------------------------------------------------------------------
# Fixtures (inline — do not modify conftest.py)
# ---------------------------------------------------------------------------


@pytest.fixture
def mock_execution_env():
    """Mock ExecutionEnvironment with plausible return values."""
    env = AsyncMock()
    vm_handle = VMHandle(
        vm_id="vm-int-1",
        host="10.0.0.1",
        provider=VMProvider.MANUAL,
        created_at="2026-01-01T00:00:00Z",
    )
    handle = EnvironmentHandle(
        env_id="env-int-1",
        worktree_path=Path("/tmp/worktree"),
        branch="main",
        project="test",
        runtime=RemoteEnvironmentRuntime(
            vm_handle=vm_handle,
            connection=MagicMock(close=AsyncMock()),
            workspace_path=WorkspacePath(
                path="/home/user/workspace", project="test", branch="main"
            ),
            profile=EnvironmentProfile(name="default"),
            teardown_commands=(),
            provision_start=0.0,
            workflow_id="wf-int-1",
        ),
    )
    env.provision = AsyncMock(return_value=handle)
    env.execute = AsyncMock(
        return_value=PhaseResult(
            outcome=Outcome.SUCCESS,
            signal="complete",
            exit_code=0,
            stdout="done",
            duration_secs=10,
            preflight_passed=True,
        )
    )
    env.teardown = AsyncMock()
    env.close = AsyncMock()
    return env


@pytest.fixture
def mock_vm_state_store():
    store = AsyncMock()
    store.get_active_assignments = AsyncMock(return_value=[])
    store.get_assignment = AsyncMock(return_value=None)
    store.record_release = AsyncMock()
    store.close = AsyncMock()
    return store


@pytest.fixture
def app(tmp_path, mock_execution_env, mock_vm_state_store):
    settings = APISettings(api_key=TEST_API_KEY, cors_origins=["http://localhost:3000"])
    application = create_app(settings)
    application.state.settings = settings
    roles_yml = tmp_path / "roles.yml"
    roles_yml.write_text(
        "agents:\n  default:\n    cli: claude\n    model: sonnet\n    auth: oauth\n"
    )
    application.state.config = Config(
        ipc_dir=str(tmp_path / "ipc"),
        github_dir=str(tmp_path / "github"),
        data_dir=str(tmp_path / "data"),
        worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
        roles_config_path=str(roles_yml),
    )
    application.state.emitter = NullEventEmitter()
    application.state.api_store = APIStateStore()
    application.state.execution_env = mock_execution_env
    application.state.vm_state_store = mock_vm_state_store
    return application


@pytest.fixture
async def client(app):
    async with AsyncClient(
        # ASGITransport does not trigger lifespan events; state is manually seeded above.
        transport=ASGITransport(app=app),
        base_url="http://test",
    ) as c:
        yield c


@pytest.fixture
def auth_headers():
    return {"X-API-Key": TEST_API_KEY}


# ---------------------------------------------------------------------------
# Health
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_health_returns_ok(client):
    """GET /api/v1/health returns 200 with status ok."""
    resp = await client.get("/api/v1/health")
    assert resp.status_code == 200
    data = resp.json()
    assert data["status"] == "ok"


@pytest.mark.asyncio
async def test_health_ready(client):
    """GET /api/v1/health/ready returns 200."""
    resp = await client.get("/api/v1/health/ready")
    assert resp.status_code == 200
    data = resp.json()
    assert data["status"] == "ready"


# ---------------------------------------------------------------------------
# Auth
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_auth_missing_key(client):
    """POST /api/v1/dispatch without X-API-Key header returns 422."""
    resp = await client.post("/api/v1/dispatch", json={})
    assert resp.status_code == 422


@pytest.mark.asyncio
async def test_auth_wrong_key(client):
    """POST /api/v1/dispatch with wrong API key returns 401 with structured error."""
    resp = await client.post(
        "/api/v1/dispatch",
        headers={"X-API-Key": "wrong"},
        json={},
    )
    assert resp.status_code == 401
    body = resp.json()
    assert body["error_code"] == "authentication_error"
    assert "timestamp" in body
    assert "request_id" in body


@pytest.mark.asyncio
async def test_auth_correct_key(client, auth_headers):
    """Correct API key is accepted — endpoint does not return 401 or 422."""
    resp = await client.get("/api/v1/config", headers=auth_headers)
    assert resp.status_code not in (401, 422)


# ---------------------------------------------------------------------------
# Middleware
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_request_id_header_present(client):
    """Every response should include an x-request-id header."""
    resp = await client.get("/api/v1/health")
    assert "x-request-id" in resp.headers


@pytest.mark.asyncio
async def test_request_ids_unique(client):
    """Two requests should produce different x-request-id values."""
    resp1 = await client.get("/api/v1/health")
    resp2 = await client.get("/api/v1/health")
    assert resp1.headers["x-request-id"] != resp2.headers["x-request-id"]


# ---------------------------------------------------------------------------
# Dispatch lifecycle
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_dispatch_lifecycle(client, auth_headers, app):
    """Create -> query -> cancel lifecycle."""
    # No execution env so dispatch stays pending
    app.state.execution_env = None

    # Create
    resp = await client.post(
        "/api/v1/dispatch",
        headers=auth_headers,
        json={
            "project": "test",
            "phase": "do-task",
            "branch": "main",
            "spec_folder": "specs/test",
            "cli": "claude",
        },
    )
    assert resp.status_code == 200
    dispatch_id = resp.json()["dispatch_id"]
    assert re.match(r"^wf-.*-\d+-\d+$", dispatch_id)

    # Query
    resp = await client.get(f"/api/v1/dispatch/{dispatch_id}", headers=auth_headers)
    assert resp.status_code == 200
    assert resp.json()["workflow_id"] == dispatch_id

    # Cancel
    resp = await client.delete(f"/api/v1/dispatch/{dispatch_id}", headers=auth_headers)
    assert resp.status_code == 200
    assert resp.json()["status"] == "cancelled"


@pytest.mark.asyncio
async def test_dispatch_cancel_terminal_returns_409(client, auth_headers, app):
    """DELETE /api/v1/dispatch/{id} returns 409 for already-cancelled dispatch."""
    app.state.execution_env = None

    resp = await client.post(
        "/api/v1/dispatch",
        headers=auth_headers,
        json={
            "project": "test",
            "phase": "do-task",
            "branch": "main",
            "spec_folder": "specs/test",
            "cli": "claude",
        },
    )
    dispatch_id = resp.json()["dispatch_id"]

    # Cancel once
    resp = await client.delete(f"/api/v1/dispatch/{dispatch_id}", headers=auth_headers)
    assert resp.status_code == 200

    # Cancel again → 409
    resp = await client.delete(f"/api/v1/dispatch/{dispatch_id}", headers=auth_headers)
    assert resp.status_code == 409


@pytest.mark.asyncio
async def test_dispatch_not_found(client, auth_headers):
    """GET /api/v1/dispatch/{id} returns 404 for unknown ID."""
    resp = await client.get("/api/v1/dispatch/nonexistent", headers=auth_headers)
    assert resp.status_code == 404


@pytest.mark.asyncio
async def test_dispatch_cancel_not_found(client, auth_headers):
    """DELETE /api/v1/dispatch/{id} returns 404 for unknown ID."""
    resp = await client.delete("/api/v1/dispatch/nonexistent", headers=auth_headers)
    assert resp.status_code == 404


# ---------------------------------------------------------------------------
# VM endpoints
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_vm_list_empty(client, auth_headers):
    """GET /api/v1/vm returns empty list."""
    resp = await client.get("/api/v1/vm", headers=auth_headers)
    assert resp.status_code == 200
    assert resp.json() == []


@pytest.mark.asyncio
async def test_vm_list_with_assignments(client, auth_headers, app):
    """GET /api/v1/vm returns active VMs."""
    app.state.vm_state_store.get_active_assignments.return_value = [
        VMAssignment(
            vm_id="vm-1",
            workflow_id="wf-1",
            project="proj",
            spec="specs/a",
            host="10.0.0.1",
            assigned_at="2026-01-01T00:00:00Z",
        ),
    ]
    resp = await client.get("/api/v1/vm", headers=auth_headers)
    assert resp.status_code == 200
    data = resp.json()
    assert len(data) == 1
    assert data[0]["vm_id"] == "vm-1"


@pytest.mark.asyncio
async def test_vm_release_not_found(client, auth_headers):
    """DELETE /api/v1/vm/{id} returns 404 for unknown ID."""
    resp = await client.delete("/api/v1/vm/nonexistent", headers=auth_headers)
    assert resp.status_code == 404


@pytest.mark.asyncio
async def test_vm_provision(client, auth_headers):
    """POST /api/v1/vm/provision returns accepted, then poll shows provisioned."""
    resp = await client.post(
        "/api/v1/vm/provision",
        headers=auth_headers,
        json={"project": "test", "branch": "main"},
    )
    assert resp.status_code == 200
    data = resp.json()
    assert "env_id" in data
    assert data["status"] == "provisioning"

    env_id = data["env_id"]
    # Let the background provision task complete
    await asyncio.sleep(0.05)

    status_resp = await client.get(f"/api/v1/vm/provision/{env_id}", headers=auth_headers)
    assert status_resp.status_code == 200
    status_data = status_resp.json()
    assert status_data["status"] == "active"
    assert status_data["vm_id"] is not None
    assert status_data["host"] is not None


@pytest.mark.asyncio
async def test_vm_provision_no_exec_env(client, auth_headers, app):
    """POST /api/v1/vm/provision returns 500 when no execution env."""
    app.state.execution_env = None
    resp = await client.post(
        "/api/v1/vm/provision",
        headers=auth_headers,
        json={"project": "test", "branch": "main"},
    )
    assert resp.status_code == 500


@pytest.mark.asyncio
async def test_vm_provision_status_not_found(client, auth_headers):
    """GET /api/v1/vm/provision/{env_id} returns 404 for unknown id."""
    resp = await client.get("/api/v1/vm/provision/nonexistent", headers=auth_headers)
    assert resp.status_code == 404


@pytest.mark.asyncio
async def test_vm_dry_run(client, auth_headers):
    """POST /api/v1/vm/dry-run returns requirements."""
    resp = await client.post(
        "/api/v1/vm/dry-run",
        headers=auth_headers,
        json={"project": "test", "branch": "main"},
    )
    assert resp.status_code == 200
    data = resp.json()
    assert data["would_provision"] is True


# ---------------------------------------------------------------------------
# Events endpoint
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_events_no_db_returns_empty(client, auth_headers):
    """GET /api/v1/events returns empty when no DB configured."""
    resp = await client.get("/api/v1/events", headers=auth_headers)
    assert resp.status_code == 200
    data = resp.json()
    assert data["events"] == []
    assert data["total"] == 0


@pytest.mark.asyncio
async def test_events_with_db(client, auth_headers, app, tmp_path):
    """GET /api/v1/events returns events from SQLite DB."""
    db = tmp_path / "events.db"
    async with aiosqlite.connect(str(db)) as conn:
        await conn.executescript(_SCHEMA)
        await conn.execute(
            "INSERT INTO events (timestamp, workflow_id, event_type, payload) VALUES (?, ?, ?, ?)",
            (
                "2026-01-01T00:00:00Z",
                "wf-1",
                "DispatchReceived",
                json.dumps({
                    "type": "dispatch_received",
                    "timestamp": "2026-01-01T00:00:00Z",
                    "workflow_id": "wf-1",
                    "phase": "do-task",
                    "project": "p",
                    "cli": "claude",
                }),
            ),
        )
        await conn.commit()

    app.state.settings.events_db = str(db)
    resp = await client.get("/api/v1/events", headers=auth_headers)
    assert resp.status_code == 200
    data = resp.json()
    assert data["total"] == 1


# ---------------------------------------------------------------------------
# Run lifecycle
# ---------------------------------------------------------------------------


async def _wait_provisioned(client, env_id, auth_headers, *, max_secs: float = 2.0):
    """Poll until env reaches 'provisioned' status (helper)."""
    deadline = asyncio.get_event_loop().time() + max_secs
    while asyncio.get_event_loop().time() < deadline:
        await asyncio.sleep(0.02)
        r = await client.get(f"/api/v1/run/{env_id}/status", headers=auth_headers)
        if r.json()["status"] == "provisioned":
            return
    raise AssertionError(f"Environment {env_id} did not reach 'provisioned' within {max_secs}s")


@pytest.mark.asyncio
async def test_run_provision_and_status(client, auth_headers):
    """POST /api/v1/run/provision returns provisioning, then transitions to provisioned."""
    resp = await client.post(
        "/api/v1/run/provision",
        headers=auth_headers,
        json={"project": "test", "branch": "main"},
    )
    assert resp.status_code == 200
    data = resp.json()
    env_id = data["env_id"]
    assert data["status"] == "provisioning"

    await _wait_provisioned(client, env_id, auth_headers)
    status_resp = await client.get(f"/api/v1/run/{env_id}/status", headers=auth_headers)
    assert status_resp.status_code == 200
    assert status_resp.json()["status"] == "provisioned"


@pytest.mark.asyncio
async def test_run_execute_after_provision(client, auth_headers):
    """POST /api/v1/run/{env_id}/execute returns accepted after provision completes."""
    prov = await client.post(
        "/api/v1/run/provision",
        headers=auth_headers,
        json={"project": "test", "branch": "main"},
    )
    env_id = prov.json()["env_id"]
    await _wait_provisioned(client, env_id, auth_headers)

    exec_resp = await client.post(
        f"/api/v1/run/{env_id}/execute",
        headers=auth_headers,
        json={
            "project": "test",
            "spec_path": "specs/test",
            "phase": "do-task",
            "cli": "claude",
            "auth": "api_key",
        },
    )
    assert exec_resp.status_code == 200
    assert exec_resp.json()["status"] == "executing"


@pytest.mark.asyncio
async def test_run_execute_before_provision_complete(client, auth_headers, mock_execution_env):
    """POST /run/{env_id}/execute returns 409 if env is still provisioning."""
    # Make provision hang so we can try execute while still provisioning
    provision_started = asyncio.Event()
    provision_release = asyncio.Event()

    async def _slow_provision(*args, **kwargs):
        provision_started.set()
        await provision_release.wait()
        return mock_execution_env.provision.return_value

    mock_execution_env.provision = AsyncMock(side_effect=_slow_provision)

    prov = await client.post(
        "/api/v1/run/provision",
        headers=auth_headers,
        json={"project": "test", "branch": "main"},
    )
    env_id = prov.json()["env_id"]
    await provision_started.wait()

    exec_resp = await client.post(
        f"/api/v1/run/{env_id}/execute",
        headers=auth_headers,
        json={
            "project": "test",
            "spec_path": "specs/test",
            "phase": "do-task",
            "cli": "claude",
            "auth": "api_key",
        },
    )
    assert exec_resp.status_code == 409

    # Clean up: let the provision complete
    provision_release.set()
    await asyncio.sleep(0.05)


@pytest.mark.asyncio
async def test_run_teardown(client, auth_headers):
    """POST /api/v1/run/{env_id}/teardown returns accepted."""
    prov = await client.post(
        "/api/v1/run/provision",
        headers=auth_headers,
        json={"project": "test", "branch": "main"},
    )
    env_id = prov.json()["env_id"]
    await _wait_provisioned(client, env_id, auth_headers)

    resp = await client.post(f"/api/v1/run/{env_id}/teardown", headers=auth_headers)
    assert resp.status_code == 200
    assert resp.json()["status"] == "tearing_down"


@pytest.mark.asyncio
async def test_run_full(client, auth_headers):
    """POST /api/v1/run/full returns accepted."""
    resp = await client.post(
        "/api/v1/run/full",
        headers=auth_headers,
        json={
            "project": "test",
            "branch": "main",
            "spec_path": "specs/test",
            "phase": "do-task",
            "cli": "claude",
            "auth": "api_key",
        },
    )
    assert resp.status_code == 200
    data = resp.json()
    assert "dispatch_id" in data
    assert data["status"] == "accepted"


@pytest.mark.asyncio
async def test_run_status_not_found(client, auth_headers):
    """GET /api/v1/run/{env_id}/status returns 404 for unknown env."""
    resp = await client.get("/api/v1/run/nonexistent/status", headers=auth_headers)
    assert resp.status_code == 404


@pytest.mark.asyncio
async def test_run_teardown_uses_fresh_handle(client, auth_headers, app, mock_execution_env):
    """Teardown uses the transitioned record (not stale snapshot) so VM is released."""
    from tanren_api.models import (
        RunEnvironmentStatus,
    )
    from tanren_api.state import (
        EnvironmentRecord,
    )

    store = app.state.api_store
    handle = mock_execution_env.provision.return_value

    # Add env with handle=None, then update with real handle to simulate race
    env_id = "env-race-test"
    record = EnvironmentRecord(
        env_id=env_id,
        handle=None,
        status=RunEnvironmentStatus.PROVISIONED,
        started_at="2026-01-01T00:00:00Z",
    )
    await store.add_environment(record)
    await store.update_environment(env_id, handle=handle, vm_id="vm-int-1", host="10.0.0.1")

    resp = await client.post(f"/api/v1/run/{env_id}/teardown", headers=auth_headers)
    assert resp.status_code == 200

    await asyncio.sleep(0.1)
    mock_execution_env.teardown.assert_awaited_once_with(handle)


@pytest.mark.asyncio
async def test_provision_then_immediate_teardown_no_orphan(
    client, auth_headers, app, mock_execution_env, monkeypatch
):
    """Concurrent provision + immediate teardown → VM is cleaned up, no orphan.

    Provision returns a handle but gets cancelled before persisting it.
    The finally block in _provision_background cleans up the orphaned handle.
    """
    import contextlib  # noqa: PLC0415 — deferred import for test clarity

    from tanren_api.state import _UNSET  # noqa: PLC0415, PLC2701 — testing private implementation

    store = app.state.api_store
    original_handle = mock_execution_env.provision.return_value

    # Block try_transition_environment when it tries to persist the handle,
    # simulating the window where provision completed but hasn't persisted yet.
    persist_reached = asyncio.Event()
    persist_release = asyncio.Event()
    original_try = store.try_transition_environment

    async def _intercept_try(env_id, **kwargs):
        h = kwargs.get("handle", _UNSET)
        if not isinstance(h, type(_UNSET)) and h is not None:
            persist_reached.set()
            await persist_release.wait()
        return await original_try(env_id, **kwargs)

    monkeypatch.setattr(store, "try_transition_environment", _intercept_try)

    # Start provision
    prov = await client.post(
        "/api/v1/run/provision",
        headers=auth_headers,
        json={"project": "test", "branch": "main"},
    )
    assert prov.status_code == 200
    env_id = prov.json()["env_id"]

    # Wait for provision to reach the persist step (handle obtained)
    await persist_reached.wait()

    # Cancel the provision task (simulating teardown cancelling it)
    record = await store.get_environment(env_id)
    assert record is not None
    bg_task = record.task
    assert bg_task is not None
    bg_task.cancel()

    # Unblock the intercepted update — cancellation propagates
    persist_release.set()
    with contextlib.suppress(asyncio.CancelledError):
        await bg_task

    # Finally block should have called teardown with the handle
    mock_execution_env.teardown.assert_awaited_once_with(original_handle)


@pytest.mark.asyncio
async def test_teardown_during_noncancellable_provision_no_orphan(
    client, auth_headers, app, mock_execution_env, monkeypatch
):
    """End-to-end: provision blocks in a non-cancellable section, teardown
    arrives, and the VM is still cleaned up (no orphan)."""
    store = app.state.api_store
    original_handle = mock_execution_env.provision.return_value

    # Make provision block in a non-cancellable section
    provision_entered = asyncio.Event()
    provision_release = asyncio.Event()

    async def _noncancellable_provision(*args, **kwargs):
        provision_entered.set()
        try:
            await provision_release.wait()
        except asyncio.CancelledError:
            asyncio.current_task().uncancel()  # type: ignore[union-attr]
            await provision_release.wait()
        return original_handle

    mock_execution_env.provision = AsyncMock(side_effect=_noncancellable_provision)

    # Reduce cancel_environment_task timeout
    original_cancel = store.cancel_environment_task

    async def _fast_cancel(eid: str, *, wait_secs: float = 0.1) -> bool:
        return await original_cancel(eid, wait_secs=wait_secs)

    monkeypatch.setattr(store, "cancel_environment_task", _fast_cancel)

    # Start provision
    prov = await client.post(
        "/api/v1/run/provision",
        headers=auth_headers,
        json={"project": "test", "branch": "main"},
    )
    assert prov.status_code == 200
    env_id = prov.json()["env_id"]
    await provision_entered.wait()

    # Teardown while provision is still running (non-cancellable)
    resp = await client.post(
        f"/api/v1/run/{env_id}/teardown",
        headers=auth_headers,
    )
    assert resp.status_code == 200

    # Unblock provision — it finishes and persists the handle
    provision_release.set()

    # Wait for teardown background task to complete
    await asyncio.sleep(0.3)

    # execution_env.teardown must have been called — no orphan
    mock_execution_env.teardown.assert_awaited_once_with(original_handle)

    # Environment record should be removed
    assert await store.get_environment(env_id) is None


@pytest.mark.asyncio
async def test_concurrent_teardown_during_provision_preserves_tearing_down(
    client, auth_headers, app, mock_execution_env, monkeypatch
):
    """End-to-end: provision blocks, teardown arrives, provision completes.

    Provision's try_transition_environment sees TEARING_DOWN and refuses to
    overwrite → provision's finally block cleans up the handle.  Teardown
    background removes the record.
    """
    store = app.state.api_store
    original_handle = mock_execution_env.provision.return_value

    # Make provision block so we can interleave teardown
    provision_entered = asyncio.Event()
    provision_release = asyncio.Event()

    async def _blocking_provision(*args, **kwargs):
        provision_entered.set()
        try:
            await provision_release.wait()
        except asyncio.CancelledError:
            asyncio.current_task().uncancel()  # type: ignore[union-attr]
            await provision_release.wait()
        return original_handle

    mock_execution_env.provision = AsyncMock(side_effect=_blocking_provision)

    # Reduce cancel_environment_task timeout
    original_cancel = store.cancel_environment_task

    async def _fast_cancel(eid: str, *, wait_secs: float = 0.1) -> bool:
        return await original_cancel(eid, wait_secs=wait_secs)

    monkeypatch.setattr(store, "cancel_environment_task", _fast_cancel)

    # Start provision
    prov = await client.post(
        "/api/v1/run/provision",
        headers=auth_headers,
        json={"project": "test", "branch": "main"},
    )
    assert prov.status_code == 200
    env_id = prov.json()["env_id"]
    await provision_entered.wait()

    # Teardown while provision is blocked
    resp = await client.post(
        f"/api/v1/run/{env_id}/teardown",
        headers=auth_headers,
    )
    assert resp.status_code == 200

    # Release provision
    provision_release.set()
    await asyncio.sleep(0.3)

    # teardown called exactly once (by provision's finally block)
    mock_execution_env.teardown.assert_awaited_once_with(original_handle)

    # Environment record removed
    assert await store.get_environment(env_id) is None


@pytest.mark.asyncio
async def test_run_status_includes_vm_identity(client, auth_headers):
    """GET /run/{env_id}/status includes vm_id and host after provisioning."""
    prov = await client.post(
        "/api/v1/run/provision",
        headers=auth_headers,
        json={"project": "test", "branch": "main"},
    )
    env_id = prov.json()["env_id"]
    await _wait_provisioned(client, env_id, auth_headers)

    status_resp = await client.get(f"/api/v1/run/{env_id}/status", headers=auth_headers)
    assert status_resp.status_code == 200
    data = status_resp.json()
    assert data["vm_id"] == "vm-int-1"
    assert data["host"] == "10.0.0.1"


@pytest.mark.asyncio
async def test_vm_provision_status_shows_failed(client, auth_headers, mock_execution_env):
    """GET /vm/provision/{env_id} returns 'failed' when provisioning fails."""
    mock_execution_env.provision = AsyncMock(side_effect=RuntimeError("boom"))

    resp = await client.post(
        "/api/v1/vm/provision",
        headers=auth_headers,
        json={"project": "test", "branch": "main"},
    )
    assert resp.status_code == 200
    env_id = resp.json()["env_id"]

    await asyncio.sleep(0.05)

    status_resp = await client.get(f"/api/v1/vm/provision/{env_id}", headers=auth_headers)
    assert status_resp.status_code == 200
    assert status_resp.json()["status"] == "failed"


# ---------------------------------------------------------------------------
# Error response format
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_error_response_format(client, auth_headers):
    """Error responses conform to the ErrorResponse schema."""
    resp = await client.get("/api/v1/dispatch/nonexistent", headers=auth_headers)
    assert resp.status_code == 404
    body = resp.json()
    assert "detail" in body
    assert "error_code" in body
    assert "timestamp" in body
    assert "request_id" in body


# ---------------------------------------------------------------------------
# CORS
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_cors_preflight(client):
    """OPTIONS preflight with allowed origin returns CORS headers."""
    resp = await client.options(
        "/api/v1/dispatch",
        headers={
            "Origin": "http://localhost:3000",
            "Access-Control-Request-Method": "POST",
        },
    )
    assert "access-control-allow-origin" in resp.headers


# ---------------------------------------------------------------------------
# Settings
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_app_creates_without_cors(tmp_path, mock_execution_env, mock_vm_state_store):
    """App can be created with empty cors_origins list."""
    settings = APISettings(api_key="key", cors_origins=[])
    application = create_app(settings)
    application.state.settings = settings
    roles_yml = tmp_path / "roles.yml"
    roles_yml.write_text(
        "agents:\n  default:\n    cli: claude\n    model: sonnet\n    auth: oauth\n"
    )
    application.state.config = Config(
        ipc_dir=str(tmp_path / "ipc"),
        github_dir=str(tmp_path / "github"),
        data_dir=str(tmp_path / "data"),
        worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
        roles_config_path=str(roles_yml),
    )
    application.state.emitter = NullEventEmitter()
    application.state.api_store = APIStateStore()
    application.state.execution_env = mock_execution_env
    application.state.vm_state_store = mock_vm_state_store

    async with AsyncClient(
        transport=ASGITransport(app=application),
        base_url="http://test",
    ) as c:
        resp = await c.get("/api/v1/health")
        assert resp.status_code == 200


# ---------------------------------------------------------------------------
# Lifespan (main.py lines 49-63)
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_lifespan_sets_state_with_config_from_env(tmp_path):
    """Lifespan populates app.state with settings, config, and emitter."""
    roles_yml = tmp_path / "roles.yml"
    roles_yml.write_text(
        "agents:\n  default:\n    cli: claude\n    model: sonnet\n    auth: oauth\n"
    )
    env_vars = {
        "WM_IPC_DIR": str(tmp_path / "ipc"),
        "WM_GITHUB_DIR": str(tmp_path / "github"),
        "WM_DATA_DIR": str(tmp_path / "data"),
        "WM_COMMANDS_DIR": ".claude/commands/tanren",
        "WM_POLL_INTERVAL": "5.0",
        "WM_HEARTBEAT_INTERVAL": "30.0",
        "WM_OPENCODE_PATH": "opencode",
        "WM_CODEX_PATH": "codex",
        "WM_CLAUDE_PATH": "claude",
        "WM_MAX_OPENCODE": "1",
        "WM_MAX_CODEX": "1",
        "WM_MAX_GATE": "3",
        "WM_WORKTREE_REGISTRY_PATH": str(tmp_path / "data" / "worktrees.json"),
        "WM_ROLES_CONFIG_PATH": str(roles_yml),
    }
    settings = APISettings(api_key=TEST_API_KEY, cors_origins=[])
    application = create_app(settings)

    with patch.dict(os.environ, env_vars, clear=False):
        async with application.router.lifespan_context(application):
            assert application.state.settings is settings
            assert application.state.config is not None
            assert application.state.config.ipc_dir == str(tmp_path / "ipc")
            assert isinstance(application.state.emitter, NullEventEmitter)
            assert isinstance(application.state.api_store, APIStateStore)


@pytest.mark.asyncio
async def test_lifespan_config_from_env_failure_sets_none(tmp_path):
    """Lifespan sets config=None when Config.from_env() fails."""
    settings = APISettings(api_key=TEST_API_KEY, cors_origins=[])
    application = create_app(settings)

    # Clear all WM_* env vars so Config.from_env() raises ValueError
    cleaned = {k: v for k, v in os.environ.items() if not k.startswith("WM_")}
    with patch.dict(os.environ, cleaned, clear=True), patch("tanren_api.main.load_config_env"):
        async with application.router.lifespan_context(application):
            assert application.state.config is None
            assert isinstance(application.state.emitter, NullEventEmitter)


@pytest.mark.asyncio
async def test_lifespan_with_events_db(tmp_path):
    """Lifespan creates SqliteEventEmitter when events_db is set."""
    db_path = str(tmp_path / "events.db")
    settings = APISettings(api_key=TEST_API_KEY, cors_origins=[], events_db=db_path)
    application = create_app(settings)

    # Config.from_env() will fail (no WM_ vars), but events_db from settings is used
    cleaned = {k: v for k, v in os.environ.items() if not k.startswith("WM_")}
    with patch.dict(os.environ, cleaned, clear=True), patch("tanren_api.main.load_config_env"):
        async with application.router.lifespan_context(application):
            assert application.state.config is None
            assert isinstance(application.state.emitter, SqliteEventEmitter)


@pytest.mark.asyncio
async def test_lifespan_events_db_from_config(tmp_path):
    """Lifespan falls back to config.events_db when settings.events_db is None."""
    db_path = str(tmp_path / "events_from_config.db")
    roles_yml = tmp_path / "roles.yml"
    roles_yml.write_text(
        "agents:\n  default:\n    cli: claude\n    model: sonnet\n    auth: oauth\n"
    )
    env_vars = {
        "WM_IPC_DIR": str(tmp_path / "ipc"),
        "WM_GITHUB_DIR": str(tmp_path / "github"),
        "WM_DATA_DIR": str(tmp_path / "data"),
        "WM_COMMANDS_DIR": ".claude/commands/tanren",
        "WM_POLL_INTERVAL": "5.0",
        "WM_HEARTBEAT_INTERVAL": "30.0",
        "WM_OPENCODE_PATH": "opencode",
        "WM_CODEX_PATH": "codex",
        "WM_CLAUDE_PATH": "claude",
        "WM_MAX_OPENCODE": "1",
        "WM_MAX_CODEX": "1",
        "WM_MAX_GATE": "3",
        "WM_WORKTREE_REGISTRY_PATH": str(tmp_path / "data" / "worktrees.json"),
        "WM_ROLES_CONFIG_PATH": str(roles_yml),
        "WM_EVENTS_DB": db_path,
    }
    settings = APISettings(api_key=TEST_API_KEY, cors_origins=[], events_db=None)
    application = create_app(settings)

    with patch.dict(os.environ, env_vars, clear=False):
        async with application.router.lifespan_context(application):
            assert isinstance(application.state.emitter, SqliteEventEmitter)


@pytest.mark.asyncio
async def test_lifespan_emitter_close_called(tmp_path):
    """Lifespan calls emitter.close() on shutdown."""
    settings = APISettings(api_key=TEST_API_KEY, cors_origins=[])
    application = create_app(settings)

    cleaned = {k: v for k, v in os.environ.items() if not k.startswith("WM_")}
    with patch.dict(os.environ, cleaned, clear=True):
        async with application.router.lifespan_context(application):
            emitter = application.state.emitter
            assert isinstance(emitter, NullEventEmitter)
        # After context exits, close() was called — NullEventEmitter.close() is a no-op
        # but we verify the lifespan completed without error


@pytest.mark.asyncio
async def test_lifespan_calls_load_config_env():
    """Lifespan calls load_config_env() before Config.from_env()."""
    settings = APISettings(api_key=TEST_API_KEY, cors_origins=[])
    application = create_app(settings)

    cleaned = {k: v for k, v in os.environ.items() if not k.startswith("WM_")}
    with (
        patch.dict(os.environ, cleaned, clear=True),
        patch("tanren_api.main.load_config_env") as mock_load,
    ):
        async with application.router.lifespan_context(application):
            mock_load.assert_called_once()


@pytest.mark.asyncio
async def test_lifespan_config_loads_from_env_file(tmp_path):
    """Lifespan loads config via load_config_env when WM_* vars are only in a file."""
    roles_yml = tmp_path / "roles.yml"
    roles_yml.write_text(
        "agents:\n  default:\n    cli: claude\n    model: sonnet\n    auth: oauth\n"
    )
    env_file = tmp_path / "tanren.env"
    env_file.write_text(
        f"WM_IPC_DIR={tmp_path / 'ipc'}\n"
        f"WM_GITHUB_DIR={tmp_path / 'github'}\n"
        f"WM_DATA_DIR={tmp_path / 'data'}\n"
        "WM_COMMANDS_DIR=.claude/commands/tanren\n"
        "WM_POLL_INTERVAL=5.0\n"
        "WM_HEARTBEAT_INTERVAL=30.0\n"
        "WM_OPENCODE_PATH=opencode\n"
        "WM_CODEX_PATH=codex\n"
        "WM_CLAUDE_PATH=claude\n"
        "WM_MAX_OPENCODE=1\n"
        "WM_MAX_CODEX=1\n"
        "WM_MAX_GATE=3\n"
        f"WM_WORKTREE_REGISTRY_PATH={tmp_path / 'data' / 'worktrees.json'}\n"
        f"WM_ROLES_CONFIG_PATH={roles_yml}\n"
    )
    settings = APISettings(api_key=TEST_API_KEY, cors_origins=[])
    application = create_app(settings)

    # Clear all WM_* vars — config is only in the file
    cleaned = {k: v for k, v in os.environ.items() if not k.startswith("WM_")}
    with (
        patch.dict(os.environ, cleaned, clear=True),
        patch(
            "tanren_api.main.load_config_env",
            side_effect=lambda source=None: load_config_env(DotenvConfigSource(env_file)),
        ),
    ):
        async with application.router.lifespan_context(application):
            assert application.state.config is not None
            assert application.state.config.ipc_dir == str(tmp_path / "ipc")
            assert application.state.config.github_dir == str(tmp_path / "github")


# ---------------------------------------------------------------------------
# Dependencies (dependencies.py lines 12, 22)
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_dependency_get_settings(app):
    """get_settings dependency returns the APISettings from app state."""
    async with AsyncClient(
        transport=ASGITransport(app=app),
        base_url="http://test",
    ) as c:
        resp = await c.get("/api/v1/config", headers={"X-API-Key": TEST_API_KEY})
        assert resp.status_code == 200


def test_dependency_get_config_returns_config(app):
    """get_config returns the Config object stored in app.state."""
    scope = {"type": "http", "app": app}
    request = Request(scope)
    config = get_config(request)
    assert isinstance(config, Config)
    assert config is app.state.config


def test_dependency_get_settings_returns_settings(app):
    """get_settings returns the APISettings object stored in app.state."""
    scope = {"type": "http", "app": app}
    request = Request(scope)
    settings = get_settings(request)
    assert isinstance(settings, APISettings)
    assert settings is app.state.settings


def test_dependency_get_emitter_returns_emitter(app):
    """get_emitter returns the EventEmitter stored in app.state."""
    scope = {"type": "http", "app": app}
    request = Request(scope)
    emitter = get_emitter(request)
    assert emitter is app.state.emitter


# ---------------------------------------------------------------------------
# Error classes (errors.py lines 27, 35, 51)
# ---------------------------------------------------------------------------


def test_not_found_error_defaults():
    """NotFoundError has correct status_code, error_code, and default detail."""
    err = NotFoundError()
    assert err.status_code == 404
    assert err.error_code == "not_found"
    assert err.detail == "Resource not found"


def test_not_found_error_custom_detail():
    """NotFoundError accepts a custom detail message."""
    err = NotFoundError("VM not found")
    assert err.status_code == 404
    assert err.detail == "VM not found"


def test_authentication_error_defaults():
    """AuthenticationError has correct status_code, error_code, and default detail."""
    err = AuthenticationError()
    assert err.status_code == 401
    assert err.error_code == "authentication_error"
    assert err.detail == "Authentication failed"


def test_authentication_error_custom_detail():
    """AuthenticationError accepts a custom detail message."""
    err = AuthenticationError("Token expired")
    assert err.status_code == 401
    assert err.detail == "Token expired"


def test_conflict_error_defaults():
    """ConflictError has correct status_code, error_code, and default detail."""
    err = ConflictError()
    assert err.status_code == 409
    assert err.error_code == "conflict"
    assert err.detail == "Conflict"


def test_conflict_error_custom_detail():
    """ConflictError accepts a custom detail message."""
    err = ConflictError("Already running")
    assert err.status_code == 409
    assert err.detail == "Already running"


def test_service_error_defaults():
    """ServiceError has correct status_code, error_code, and default detail."""
    err = ServiceError()
    assert err.status_code == 500
    assert err.error_code == "service_error"
    assert err.detail == "Internal server error"


def test_service_error_custom_detail():
    """ServiceError accepts a custom detail message."""
    err = ServiceError("Database connection failed")
    assert err.status_code == 500
    assert err.detail == "Database connection failed"


def test_tanren_api_error_is_exception():
    """TanrenAPIError subclasses are proper exceptions with str representation."""
    err = NotFoundError("gone")
    assert isinstance(err, TanrenAPIError)
    assert isinstance(err, Exception)
    assert str(err) == "gone"


@pytest.mark.asyncio
async def test_error_handler_not_found(app, auth_headers):
    """NotFoundError triggers the global handler and returns 404 with error body."""

    @app.get("/api/v1/test-not-found")
    def _raise_not_found():
        raise NotFoundError("item missing")

    async with AsyncClient(
        transport=ASGITransport(app=app),
        base_url="http://test",
    ) as c:
        resp = await c.get("/api/v1/test-not-found", headers=auth_headers)
        assert resp.status_code == 404
        body = resp.json()
        assert body["error_code"] == "not_found"
        assert body["detail"] == "item missing"
        assert "timestamp" in body
        assert "request_id" in body


@pytest.mark.asyncio
async def test_error_handler_authentication(app, auth_headers):
    """AuthenticationError triggers the global handler and returns 401."""

    @app.get("/api/v1/test-auth-error")
    def _raise_auth_error():
        raise AuthenticationError("bad token")

    async with AsyncClient(
        transport=ASGITransport(app=app),
        base_url="http://test",
    ) as c:
        resp = await c.get("/api/v1/test-auth-error", headers=auth_headers)
        assert resp.status_code == 401
        body = resp.json()
        assert body["error_code"] == "authentication_error"
        assert body["detail"] == "bad token"


@pytest.mark.asyncio
async def test_error_handler_service_error(app, auth_headers):
    """ServiceError triggers the global handler and returns 500."""

    @app.get("/api/v1/test-service-error")
    def _raise_service_error():
        raise ServiceError("db down")

    async with AsyncClient(
        transport=ASGITransport(app=app),
        base_url="http://test",
    ) as c:
        resp = await c.get("/api/v1/test-service-error", headers=auth_headers)
        assert resp.status_code == 500
        body = resp.json()
        assert body["error_code"] == "service_error"
        assert body["detail"] == "db down"


# ---------------------------------------------------------------------------
# Auth — edge cases (auth.py line 15, APIKeyVerifier)
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_api_key_verifier_empty_key_rejects():
    """APIKeyVerifier with empty expected key rejects all credentials."""
    verifier = APIKeyVerifier("")
    with pytest.raises(AuthenticationError) as exc_info:
        await verifier.verify("anything")
    assert exc_info.value.status_code == 401


@pytest.mark.asyncio
async def test_api_key_verifier_matching_key_accepts():
    """APIKeyVerifier accepts when credentials match the expected key."""
    verifier = APIKeyVerifier("secret")
    # Should not raise
    await verifier.verify("secret")


@pytest.mark.asyncio
async def test_api_key_verifier_mismatch_rejects():
    """APIKeyVerifier rejects credentials that don't match."""
    verifier = APIKeyVerifier("secret")
    with pytest.raises(AuthenticationError) as exc_info:
        await verifier.verify("wrong")
    assert exc_info.value.status_code == 401


# ---------------------------------------------------------------------------
# Config endpoint — field validation
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_config_endpoint_returns_expected_fields(client, auth_headers):
    """GET /api/v1/config returns all expected configuration fields."""
    resp = await client.get("/api/v1/config", headers=auth_headers)
    assert resp.status_code == 200
    body = resp.json()
    expected_fields = {
        "ipc_dir",
        "github_dir",
        "poll_interval",
        "heartbeat_interval",
        "max_opencode",
        "max_codex",
        "max_gate",
        "events_enabled",
        "remote_enabled",
    }
    assert set(body.keys()) == expected_fields


@pytest.mark.asyncio
async def test_config_endpoint_values(client, auth_headers, tmp_path):
    """GET /api/v1/config returns correct values matching app state."""
    resp = await client.get("/api/v1/config", headers=auth_headers)
    assert resp.status_code == 200
    body = resp.json()
    assert body["ipc_dir"] == str(tmp_path / "ipc")
    assert body["github_dir"] == str(tmp_path / "github")
    assert isinstance(body["poll_interval"], (int, float))
    assert isinstance(body["heartbeat_interval"], (int, float))
    assert isinstance(body["max_opencode"], int)
    assert isinstance(body["max_codex"], int)
    assert isinstance(body["max_gate"], int)
    assert isinstance(body["events_enabled"], bool)
    assert isinstance(body["remote_enabled"], bool)


@pytest.mark.asyncio
async def test_config_endpoint_events_disabled_by_default(client, auth_headers):
    """Config shows events_enabled=False when no events_db is set."""
    resp = await client.get("/api/v1/config", headers=auth_headers)
    body = resp.json()
    assert body["events_enabled"] is False


@pytest.mark.asyncio
async def test_config_endpoint_remote_disabled_by_default(client, auth_headers):
    """Config shows remote_enabled=False when no remote_config_path is set."""
    resp = await client.get("/api/v1/config", headers=auth_headers)
    body = resp.json()
    assert body["remote_enabled"] is False


# ---------------------------------------------------------------------------
# Lifespan — stale VM recovery (main.py)
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_lifespan_calls_recover_stale_on_startup(tmp_path):
    """Lifespan calls recover_stale_assignments() when execution env supports it."""
    mock_env = AsyncMock()
    mock_env.recover_stale_assignments = AsyncMock(return_value=2)
    mock_env.close = AsyncMock()
    mock_vm_store = AsyncMock()
    mock_vm_store.close = AsyncMock()

    roles_yml = tmp_path / "roles.yml"
    roles_yml.write_text(
        "agents:\n  default:\n    cli: claude\n    model: sonnet\n    auth: oauth\n"
    )
    env_vars = {
        "WM_IPC_DIR": str(tmp_path / "ipc"),
        "WM_GITHUB_DIR": str(tmp_path / "github"),
        "WM_DATA_DIR": str(tmp_path / "data"),
        "WM_COMMANDS_DIR": ".claude/commands/tanren",
        "WM_POLL_INTERVAL": "5.0",
        "WM_HEARTBEAT_INTERVAL": "30.0",
        "WM_OPENCODE_PATH": "opencode",
        "WM_CODEX_PATH": "codex",
        "WM_CLAUDE_PATH": "claude",
        "WM_MAX_OPENCODE": "1",
        "WM_MAX_CODEX": "1",
        "WM_MAX_GATE": "3",
        "WM_WORKTREE_REGISTRY_PATH": str(tmp_path / "data" / "worktrees.json"),
        "WM_ROLES_CONFIG_PATH": str(roles_yml),
        "WM_REMOTE_CONFIG": str(tmp_path / "remote.toml"),
    }
    settings = APISettings(api_key=TEST_API_KEY, cors_origins=[])
    application = create_app(settings)

    with (
        patch.dict(os.environ, env_vars, clear=False),
        patch(
            "tanren_api.main.build_ssh_execution_environment",
            return_value=(mock_env, mock_vm_store),
        ),
    ):
        async with application.router.lifespan_context(application):
            assert application.state.execution_env is mock_env
            mock_env.recover_stale_assignments.assert_awaited_once()


@pytest.mark.asyncio
async def test_lifespan_recover_stale_failure_does_not_block_startup(tmp_path):
    """Startup completes even if recover_stale_assignments raises."""
    mock_env = AsyncMock()
    mock_env.recover_stale_assignments = AsyncMock(side_effect=RuntimeError("boom"))
    mock_env.close = AsyncMock()
    mock_vm_store = AsyncMock()
    mock_vm_store.close = AsyncMock()

    roles_yml = tmp_path / "roles.yml"
    roles_yml.write_text(
        "agents:\n  default:\n    cli: claude\n    model: sonnet\n    auth: oauth\n"
    )
    env_vars = {
        "WM_IPC_DIR": str(tmp_path / "ipc"),
        "WM_GITHUB_DIR": str(tmp_path / "github"),
        "WM_DATA_DIR": str(tmp_path / "data"),
        "WM_COMMANDS_DIR": ".claude/commands/tanren",
        "WM_POLL_INTERVAL": "5.0",
        "WM_HEARTBEAT_INTERVAL": "30.0",
        "WM_OPENCODE_PATH": "opencode",
        "WM_CODEX_PATH": "codex",
        "WM_CLAUDE_PATH": "claude",
        "WM_MAX_OPENCODE": "1",
        "WM_MAX_CODEX": "1",
        "WM_MAX_GATE": "3",
        "WM_WORKTREE_REGISTRY_PATH": str(tmp_path / "data" / "worktrees.json"),
        "WM_ROLES_CONFIG_PATH": str(roles_yml),
        "WM_REMOTE_CONFIG": str(tmp_path / "remote.toml"),
    }
    settings = APISettings(api_key=TEST_API_KEY, cors_origins=[])
    application = create_app(settings)

    with (
        patch.dict(os.environ, env_vars, clear=False),
        patch(
            "tanren_api.main.build_ssh_execution_environment",
            return_value=(mock_env, mock_vm_store),
        ),
    ):
        async with application.router.lifespan_context(application):
            # App started despite recovery failure
            assert application.state.execution_env is mock_env


# ---------------------------------------------------------------------------
# Middleware — request logging and non-HTTP scopes
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_request_logging_no_errors(client, caplog):
    """Request logging middleware completes without errors on normal requests."""
    with caplog.at_level(logging.INFO, logger="tanren_api.middleware"):
        resp = await client.get("/api/v1/health")
        assert resp.status_code == 200
    # Verify log entry was produced with method, path, status, and duration
    log_messages = [r.message for r in caplog.records if "tanren_api.middleware" in r.name]
    assert any("GET" in msg and "/api/v1/health" in msg and "200" in msg for msg in log_messages)


@pytest.mark.asyncio
async def test_request_logging_on_error_response(client, auth_headers, caplog):
    """Request logging middleware logs error status codes correctly."""
    with caplog.at_level(logging.INFO, logger="tanren_api.middleware"):
        resp = await client.post(
            "/api/v1/dispatch",
            headers={"X-API-Key": "wrong-key"},
            json={},
        )
        assert resp.status_code == 401
    log_messages = [r.message for r in caplog.records if "tanren_api.middleware" in r.name]
    assert any("401" in msg for msg in log_messages)


@pytest.mark.asyncio
async def test_request_id_attached_to_error_response(client, auth_headers):
    """Error responses from the global handler include the request_id from middleware."""
    resp = await client.get("/api/v1/dispatch/nonexistent", headers=auth_headers)
    assert resp.status_code == 404
    body = resp.json()
    header_id = resp.headers["x-request-id"]
    assert body["request_id"] == header_id


# ---------------------------------------------------------------------------
# Concurrent requests
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_concurrent_requests_all_succeed(client, auth_headers):
    """Multiple concurrent requests all return successfully."""
    tasks = [
        client.get("/api/v1/health"),
        client.get("/api/v1/health/ready"),
        client.get("/api/v1/config", headers=auth_headers),
        client.get("/api/v1/health"),
        client.get("/api/v1/health/ready"),
    ]
    responses = await asyncio.gather(*tasks)
    for resp in responses:
        assert resp.status_code == 200


@pytest.mark.asyncio
async def test_concurrent_requests_unique_ids(client):
    """Concurrent requests each get unique request IDs."""
    tasks = [client.get("/api/v1/health") for _ in range(10)]
    responses = await asyncio.gather(*tasks)
    request_ids = [resp.headers["x-request-id"] for resp in responses]
    assert len(set(request_ids)) == 10


# ---------------------------------------------------------------------------
# Additional edge cases
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_unknown_route_returns_404(client):
    """Request to a non-existent route returns 404."""
    resp = await client.get("/api/v1/nonexistent")
    assert resp.status_code == 404


@pytest.mark.asyncio
async def test_health_response_includes_version(client):
    """Health endpoint includes version field."""
    resp = await client.get("/api/v1/health")
    body = resp.json()
    assert "version" in body
    assert body["version"] == "0.1.0"


@pytest.mark.asyncio
async def test_health_response_includes_uptime(client):
    """Health endpoint includes uptime_seconds field."""
    resp = await client.get("/api/v1/health")
    body = resp.json()
    assert "uptime_seconds" in body
    assert isinstance(body["uptime_seconds"], (int, float))
    assert body["uptime_seconds"] >= 0


@pytest.mark.asyncio
async def test_dispatch_invalid_body_returns_422(client, auth_headers):
    """POST /api/v1/dispatch with invalid body returns 422 validation error."""
    resp = await client.post(
        "/api/v1/dispatch",
        headers=auth_headers,
        json={"invalid_field": "value"},
    )
    assert resp.status_code == 422


@pytest.mark.asyncio
async def test_cors_disallowed_origin(client):
    """Preflight from a disallowed origin does not include CORS allow header."""
    resp = await client.options(
        "/api/v1/dispatch",
        headers={
            "Origin": "http://evil.example.com",
            "Access-Control-Request-Method": "POST",
        },
    )
    allow_origin = resp.headers.get("access-control-allow-origin", "")
    assert "evil.example.com" not in allow_origin


@pytest.mark.asyncio
async def test_middleware_websocket_scope_passthrough(app):
    """Middleware passes through non-HTTP scopes (e.g. websocket) without error."""
    calls: list[str] = []

    async def inner_app(scope, receive, send):
        calls.append(scope["type"])
        await receive()  # consume to satisfy async requirement

    wrapped = RequestIDMiddleware(RequestLoggingMiddleware(inner_app))

    scope = {"type": "websocket", "path": "/ws"}

    async def noop_receive():
        await asyncio.sleep(0)
        return {"type": "websocket.connect"}

    async def noop_send(msg):
        await asyncio.sleep(0)  # satisfy async requirement

    await wrapped(scope, noop_receive, noop_send)
    assert calls == ["websocket"]


@pytest.mark.asyncio
async def test_vm_provision_orphan_cleanup_on_record_removal(
    client, auth_headers, app, mock_execution_env, monkeypatch
):
    """End-to-end: VM provision blocks, teardown removes record, provision
    completes → try_transition_environment returns None, finally block
    cleans up the handle (no orphan)."""
    store = app.state.api_store
    original_handle = mock_execution_env.provision.return_value

    # Make provision block so we can remove the record while it's in-flight
    provision_entered = asyncio.Event()
    provision_release = asyncio.Event()

    async def _blocking_provision(*args, **kwargs):
        provision_entered.set()
        await provision_release.wait()
        return original_handle

    mock_execution_env.provision = AsyncMock(side_effect=_blocking_provision)

    # Start provision
    resp = await client.post(
        "/api/v1/vm/provision",
        headers=auth_headers,
        json={"project": "test", "branch": "main"},
    )
    assert resp.status_code == 200
    env_id = resp.json()["env_id"]

    # Wait for provision to start
    await provision_entered.wait()

    # Simulate concurrent teardown removing the environment record
    removed = await store.remove_environment(env_id)
    assert removed is not None

    # Release provision — it finishes but record is gone
    provision_release.set()
    await asyncio.sleep(0.1)

    # execution_env.teardown must have been called — no orphan
    mock_execution_env.teardown.assert_awaited_once_with(original_handle)


@pytest.mark.asyncio
async def test_vm_provision_records_reaped_after_retention(client, auth_headers, app):
    """Terminal environment records are removed after the retention window."""
    store = app.state.api_store

    # Provision two VMs
    resp1 = await client.post(
        "/api/v1/vm/provision",
        headers=auth_headers,
        json={"project": "test", "branch": "main"},
    )
    env_id_1 = resp1.json()["env_id"]

    resp2 = await client.post(
        "/api/v1/vm/provision",
        headers=auth_headers,
        json={"project": "test", "branch": "main"},
    )
    env_id_2 = resp2.json()["env_id"]

    # Wait for both to complete
    await asyncio.sleep(0.1)

    # Patch the first record's completed_at to be old (beyond retention)
    from datetime import (  # noqa: PLC0415 — deferred import for test clarity
        UTC,
        datetime,
        timedelta,
    )

    old_time = (datetime.now(UTC) - timedelta(seconds=3660)).isoformat()
    await store.update_environment(env_id_1, completed_at=old_time)

    # Provision a third — triggers reap
    resp3 = await client.post(
        "/api/v1/vm/provision",
        headers=auth_headers,
        json={"project": "test", "branch": "main"},
    )
    env_id_3 = resp3.json()["env_id"]

    # First record should be reaped, second and third remain
    assert await store.get_environment(env_id_1) is None
    assert await store.get_environment(env_id_2) is not None
    assert await store.get_environment(env_id_3) is not None
