"""Store protocol tests — Postgres backend (integration tests).

Uses the same shared test base as the SQLite unit tests.  Gated by
``@pytest.mark.postgres``; pass ``--postgres-url`` to run.
"""

from __future__ import annotations

import pytest
from sqlalchemy import text

from tanren_core.store.factory import create_store
from tests.store._shared import (
    SharedApiKeyTests,
    SharedEventStoreTests,
    SharedJobQueueTests,
    SharedLifecycleTests,
    SharedResourceLimitTests,
    SharedStateStoreTests,
    SharedUserTests,
)

pytestmark = pytest.mark.postgres


@pytest.fixture
async def store(request):
    pg_url = request.config.getoption("--postgres-url")
    if not pg_url:
        raise pytest.skip.Exception("--postgres-url not provided")

    s = await create_store(pg_url)

    # Clean tables before each test (order respects FK constraints)
    async with s._engine.begin() as conn:
        for table in [
            "step_projection",
            "api_key_projection",
            "user_projection",
            "dispatch_projection",
            "events",
            "vm_assignments",
        ]:
            await conn.execute(text(f"DELETE FROM {table}"))  # noqa: S608

    yield s

    # Clean after test
    async with s._engine.begin() as conn:
        for table in [
            "step_projection",
            "api_key_projection",
            "user_projection",
            "dispatch_projection",
            "events",
            "vm_assignments",
        ]:
            await conn.execute(text(f"DELETE FROM {table}"))  # noqa: S608

    await s.close()


class TestEventStore(SharedEventStoreTests):
    pass


class TestJobQueue(SharedJobQueueTests):
    pass


class TestStateStore(SharedStateStoreTests):
    pass


class TestLifecycle(SharedLifecycleTests):
    pass


class TestUsers(SharedUserTests):
    pass


class TestApiKeys(SharedApiKeyTests):
    pass


class TestResourceLimits(SharedResourceLimitTests):
    pass
