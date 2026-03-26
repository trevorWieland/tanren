"""Shared fixtures for API tests."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest
from httpx import ASGITransport, AsyncClient

from tanren_api.auth_seed import seed_legacy_admin_key
from tanren_api.main import create_app
from tanren_api.settings import APISettings
from tanren_core.store.sqlite import SqliteStore

if TYPE_CHECKING:
    from pathlib import Path

TEST_API_KEY = "tnrn_testpfx1_secretrandompartforunittesting"


@pytest.fixture
def api_settings():
    return APISettings(api_key=TEST_API_KEY, cors_origins=["*"])


@pytest.fixture
async def sqlite_store(tmp_path: Path):
    store = SqliteStore(tmp_path / "test.db")
    await store._ensure_conn()
    yield store
    await store.close()


@pytest.fixture
async def app(api_settings, tmp_path, sqlite_store):
    api_settings.db_url = str(tmp_path / "test.db")
    application = create_app(api_settings)
    application.state.settings = api_settings
    application.state.event_store = sqlite_store
    application.state.job_queue = sqlite_store
    application.state.state_store = sqlite_store
    application.state.auth_store = sqlite_store
    # Seed the admin user/key so auth works in tests
    await seed_legacy_admin_key(sqlite_store, sqlite_store, TEST_API_KEY)
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
