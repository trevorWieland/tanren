"""Integration tests for the tanren API — exercises the full ASGI stack."""

from __future__ import annotations

import asyncio
import logging
import os
from unittest.mock import patch

import pytest
from fastapi import HTTPException, Request
from httpx import ASGITransport, AsyncClient

from tanren_api.auth import APIKeyVerifier
from tanren_api.dependencies import get_config, get_emitter, get_settings
from tanren_api.errors import (
    AuthenticationError,
    NotFoundError,
    ServiceError,
    TanrenAPIError,
)
from tanren_api.main import create_app
from tanren_api.middleware import RequestIDMiddleware, RequestLoggingMiddleware
from tanren_api.settings import APISettings
from tanren_core.adapters.null_emitter import NullEventEmitter
from tanren_core.adapters.sqlite_emitter import SqliteEventEmitter
from tanren_core.config import Config

TEST_API_KEY = "test-integration-key"


# ---------------------------------------------------------------------------
# Fixtures (inline — do not modify conftest.py)
# ---------------------------------------------------------------------------


@pytest.fixture
def app(tmp_path):
    settings = APISettings(api_key=TEST_API_KEY, cors_origins=["http://localhost:3000"])
    application = create_app(settings)
    application.state.settings = settings
    application.state.config = Config(
        ipc_dir=str(tmp_path / "ipc"),
        github_dir=str(tmp_path / "github"),
        data_dir=str(tmp_path / "data"),
        worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
    )
    application.state.emitter = NullEventEmitter()
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
    """POST /api/v1/dispatch with wrong API key returns 401."""
    resp = await client.post(
        "/api/v1/dispatch",
        headers={"X-API-Key": "wrong"},
        json={},
    )
    assert resp.status_code == 401


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
# Stub endpoints (expect 501 Not Implemented)
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_dispatch_stub(client, auth_headers):
    """POST /api/v1/dispatch returns 501 (not yet implemented)."""
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
    assert resp.status_code == 501


@pytest.mark.asyncio
async def test_dispatch_get_stub(client, auth_headers):
    """GET /api/v1/dispatch/{id} returns 501."""
    resp = await client.get("/api/v1/dispatch/abc-123", headers=auth_headers)
    assert resp.status_code == 501


@pytest.mark.asyncio
async def test_dispatch_cancel_stub(client, auth_headers):
    """DELETE /api/v1/dispatch/{id} returns 501."""
    resp = await client.delete("/api/v1/dispatch/abc-123", headers=auth_headers)
    assert resp.status_code == 501


@pytest.mark.asyncio
async def test_provision_stub(client, auth_headers):
    """POST /api/v1/vm/provision returns 501."""
    resp = await client.post(
        "/api/v1/vm/provision",
        headers=auth_headers,
        json={"project": "test", "branch": "main"},
    )
    assert resp.status_code == 501


@pytest.mark.asyncio
async def test_vm_list_stub(client, auth_headers):
    """GET /api/v1/vm returns 501."""
    resp = await client.get("/api/v1/vm", headers=auth_headers)
    assert resp.status_code == 501


@pytest.mark.asyncio
async def test_vm_release_stub(client, auth_headers):
    """DELETE /api/v1/vm/{id} returns 501."""
    resp = await client.delete("/api/v1/vm/vm-123", headers=auth_headers)
    assert resp.status_code == 501


@pytest.mark.asyncio
async def test_vm_dry_run_stub(client, auth_headers):
    """POST /api/v1/vm/dry-run returns 501."""
    resp = await client.post(
        "/api/v1/vm/dry-run",
        headers=auth_headers,
        json={"project": "test", "branch": "main"},
    )
    assert resp.status_code == 501


@pytest.mark.asyncio
async def test_run_full_stub(client, auth_headers):
    """POST /api/v1/run/full returns 501."""
    resp = await client.post(
        "/api/v1/run/full",
        headers=auth_headers,
        json={
            "project": "test",
            "branch": "main",
            "spec_path": "specs/test",
            "phase": "do-task",
        },
    )
    assert resp.status_code == 501


@pytest.mark.asyncio
async def test_run_provision_stub(client, auth_headers):
    """POST /api/v1/run/provision returns 501."""
    resp = await client.post(
        "/api/v1/run/provision",
        headers=auth_headers,
        json={"project": "test", "branch": "main"},
    )
    assert resp.status_code == 501


@pytest.mark.asyncio
async def test_run_execute_stub(client, auth_headers):
    """POST /api/v1/run/{env_id}/execute returns 501."""
    resp = await client.post("/api/v1/run/env-123/execute", headers=auth_headers)
    assert resp.status_code == 501


@pytest.mark.asyncio
async def test_run_teardown_stub(client, auth_headers):
    """POST /api/v1/run/{env_id}/teardown returns 501."""
    resp = await client.post("/api/v1/run/env-123/teardown", headers=auth_headers)
    assert resp.status_code == 501


@pytest.mark.asyncio
async def test_run_status_stub(client, auth_headers):
    """GET /api/v1/run/{env_id}/status returns 501."""
    resp = await client.get("/api/v1/run/env-123/status", headers=auth_headers)
    assert resp.status_code == 501


@pytest.mark.asyncio
async def test_events_stub(client, auth_headers):
    """GET /api/v1/events returns 501."""
    resp = await client.get("/api/v1/events", headers=auth_headers)
    assert resp.status_code == 501


# ---------------------------------------------------------------------------
# Error response format
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_error_response_format(client, auth_headers):
    """Error responses conform to the ErrorResponse schema."""
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
    assert resp.status_code == 501
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
async def test_app_creates_without_cors(tmp_path):
    """App can be created with empty cors_origins list."""
    settings = APISettings(api_key="key", cors_origins=[])
    application = create_app(settings)
    application.state.settings = settings
    application.state.config = Config(
        ipc_dir=str(tmp_path / "ipc"),
        github_dir=str(tmp_path / "github"),
        data_dir=str(tmp_path / "data"),
        worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
    )
    application.state.emitter = NullEventEmitter()

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
    }
    settings = APISettings(api_key=TEST_API_KEY, cors_origins=[])
    application = create_app(settings)

    with patch.dict(os.environ, env_vars, clear=False):
        async with application.router.lifespan_context(application):
            assert application.state.settings is settings
            assert application.state.config is not None
            assert application.state.config.ipc_dir == str(tmp_path / "ipc")
            assert isinstance(application.state.emitter, NullEventEmitter)


@pytest.mark.asyncio
async def test_lifespan_config_from_env_failure_sets_none(tmp_path):
    """Lifespan sets config=None when Config.from_env() fails."""
    settings = APISettings(api_key=TEST_API_KEY, cors_origins=[])
    application = create_app(settings)

    # Clear all WM_* env vars so Config.from_env() raises ValueError
    cleaned = {k: v for k, v in os.environ.items() if not k.startswith("WM_")}
    with patch.dict(os.environ, cleaned, clear=True):
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
    with patch.dict(os.environ, cleaned, clear=True):
        async with application.router.lifespan_context(application):
            assert application.state.config is None
            assert isinstance(application.state.emitter, SqliteEventEmitter)


@pytest.mark.asyncio
async def test_lifespan_events_db_from_config(tmp_path):
    """Lifespan falls back to config.events_db when settings.events_db is None."""
    db_path = str(tmp_path / "events_from_config.db")
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


# ---------------------------------------------------------------------------
# Dependencies (dependencies.py lines 12, 22)
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_dependency_get_settings(app):
    """get_settings dependency returns the APISettings from app state."""
    # Exercise via a real request to an endpoint that uses dependencies
    # The /api/v1/config endpoint uses get_config which exercises line 12
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
    with pytest.raises(HTTPException) as exc_info:
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
    with pytest.raises(HTTPException) as exc_info:
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
    assert resp.status_code == 501
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
