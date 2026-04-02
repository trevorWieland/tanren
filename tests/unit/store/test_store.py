"""Store protocol tests — SQLite backend (unit tests).

Uses the shared test base from ``tests/store/_shared.py``.  Each test
class inherits the shared protocol tests and provides a SQLite-backed
``store`` fixture via ``tmp_path``.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

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

if TYPE_CHECKING:
    from pathlib import Path


@pytest.fixture
async def store(tmp_path: Path):
    s = await create_store(str(tmp_path / "test.db"))
    yield s
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
