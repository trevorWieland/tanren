"""Shared fixtures for API tests."""

import pytest
from httpx import ASGITransport, AsyncClient

from tanren_api.main import create_app
from tanren_api.settings import APISettings
from tanren_core.adapters.null_emitter import NullEventEmitter
from tanren_core.config import Config

TEST_API_KEY = "test-api-key-12345"


@pytest.fixture
def api_settings():
    return APISettings(api_key=TEST_API_KEY, cors_origins=["*"])


@pytest.fixture
def app(api_settings, tmp_path):
    application = create_app(api_settings)
    # Manually set up state that lifespan would normally configure,
    # since ASGITransport doesn't trigger lifespan events.
    application.state.settings = api_settings
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
        transport=ASGITransport(app=app),
        base_url="http://test",
    ) as c:
        yield c


@pytest.fixture
def auth_headers():
    return {"X-API-Key": TEST_API_KEY}
