"""Shared fixtures for API tests."""

from __future__ import annotations

from pathlib import Path
from unittest.mock import AsyncMock, MagicMock

import pytest
from httpx import ASGITransport, AsyncClient

from tanren_api.main import create_app
from tanren_api.settings import APISettings
from tanren_api.state import APIStateStore
from tanren_core.adapters.null_emitter import NullEventEmitter
from tanren_core.adapters.remote_types import VMHandle, VMProvider, WorkspacePath
from tanren_core.adapters.types import (
    EnvironmentHandle,
    PhaseResult,
    RemoteEnvironmentRuntime,
)
from tanren_core.config import Config
from tanren_core.env.environment_schema import EnvironmentProfile
from tanren_core.schemas import Outcome

TEST_API_KEY = "test-api-key-12345"


@pytest.fixture
def api_settings():
    return APISettings(api_key=TEST_API_KEY, cors_origins=["*"])


@pytest.fixture
def mock_execution_env():
    """Mock ExecutionEnvironment with plausible return values."""
    env = AsyncMock()

    # provision() returns an EnvironmentHandle
    vm_handle = VMHandle(
        vm_id="vm-test-1",
        host="10.0.0.1",
        provider=VMProvider.MANUAL,
        created_at="2026-01-01T00:00:00Z",
    )
    handle = EnvironmentHandle(
        env_id="env-test-1",
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
            workflow_id="wf-test-1",
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
    """Mock SqliteVMStateStore with empty assignments by default."""
    store = AsyncMock()
    store.get_active_assignments = AsyncMock(return_value=[])
    store.get_assignment = AsyncMock(return_value=None)
    store.record_release = AsyncMock()
    store.close = AsyncMock()
    return store


@pytest.fixture
def app(api_settings, tmp_path, mock_execution_env, mock_vm_state_store):
    application = create_app(api_settings)
    # Manually set up state that lifespan would normally configure,
    # since ASGITransport doesn't trigger lifespan events.
    application.state.settings = api_settings
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
