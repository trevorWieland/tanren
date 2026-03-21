"""Tests for the queue-based DispatchServiceV2."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

from tanren_api.models import DispatchRequest, DispatchRunStatus
from tanren_api.services.dispatch_v2 import DispatchServiceV2
from tanren_core.env.environment_schema import EnvironmentProfile
from tanren_core.schemas import AuthMode, Cli, Phase
from tanren_core.store.enums import DispatchStatus
from tanren_core.store.sqlite import SqliteStore

if TYPE_CHECKING:
    from pathlib import Path

DEFAULT_PROFILE = EnvironmentProfile(name="default")


@pytest.fixture
async def store(tmp_path: Path):
    s = SqliteStore(tmp_path / "test.db")
    await s._ensure_conn()
    yield s
    await s.close()


def _make_request(**overrides: object) -> DispatchRequest:
    defaults: dict[str, object] = {
        "project": "test",
        "phase": Phase.DO_TASK,
        "branch": "main",
        "spec_folder": "spec/001",
        "cli": Cli.CLAUDE,
        "auth": AuthMode.API_KEY,
        "timeout": 1800,
        "resolved_profile": DEFAULT_PROFILE,
    }
    return DispatchRequest.model_validate(defaults | overrides)


class TestDispatchServiceV2Create:
    async def test_create_returns_dispatch_id(self, store: SqliteStore) -> None:
        svc = DispatchServiceV2(event_store=store, job_queue=store, state_store=store)
        result = await svc.create(_make_request())
        assert result.dispatch_id.startswith("wf-test-")

    async def test_create_enqueues_provision_step(self, store: SqliteStore) -> None:
        svc = DispatchServiceV2(event_store=store, job_queue=store, state_store=store)
        accepted = await svc.create(_make_request())

        steps = await store.get_steps_for_dispatch(accepted.dispatch_id)
        assert len(steps) == 1
        assert steps[0].step_type.value == "provision"

    async def test_create_sets_dispatch_to_running(self, store: SqliteStore) -> None:
        svc = DispatchServiceV2(event_store=store, job_queue=store, state_store=store)
        accepted = await svc.create(_make_request())

        view = await store.get_dispatch(accepted.dispatch_id)
        assert view is not None
        assert view.status == DispatchStatus.RUNNING

    async def test_create_appends_dispatch_created_event(self, store: SqliteStore) -> None:
        svc = DispatchServiceV2(event_store=store, job_queue=store, state_store=store)
        accepted = await svc.create(_make_request())

        events = await store.query_events(dispatch_id=accepted.dispatch_id)
        types = [e.event_type for e in events.events]
        assert "DispatchCreated" in types
        assert "StepEnqueued" in types


class TestDispatchServiceV2Get:
    async def test_get_returns_dispatch_detail(self, store: SqliteStore) -> None:
        svc = DispatchServiceV2(event_store=store, job_queue=store, state_store=store)
        accepted = await svc.create(_make_request())

        detail = await svc.get(accepted.dispatch_id)
        assert detail.workflow_id == accepted.dispatch_id
        assert detail.project == "test"
        assert detail.status == DispatchRunStatus.RUNNING

    async def test_get_not_found(self, store: SqliteStore) -> None:
        svc = DispatchServiceV2(event_store=store, job_queue=store, state_store=store)
        from tanren_api.errors import NotFoundError

        with pytest.raises(NotFoundError):
            await svc.get("nonexistent")


class TestDispatchServiceV2Cancel:
    async def test_cancel_pending_dispatch(self, store: SqliteStore) -> None:
        svc = DispatchServiceV2(event_store=store, job_queue=store, state_store=store)

        # Create a dispatch and manually set it back to pending
        accepted = await svc.create(_make_request())
        await store.update_dispatch_status(accepted.dispatch_id, DispatchStatus.PENDING)

        cancelled = await svc.cancel(accepted.dispatch_id)
        assert cancelled.dispatch_id == accepted.dispatch_id

        view = await store.get_dispatch(accepted.dispatch_id)
        assert view is not None
        assert view.status == DispatchStatus.CANCELLED

    async def test_cancel_completed_dispatch_raises_conflict(self, store: SqliteStore) -> None:
        from tanren_api.errors import ConflictError
        from tanren_core.schemas import Outcome

        svc = DispatchServiceV2(event_store=store, job_queue=store, state_store=store)
        accepted = await svc.create(_make_request())
        await store.update_dispatch_status(
            accepted.dispatch_id,
            DispatchStatus.COMPLETED,
            Outcome.SUCCESS,
        )

        with pytest.raises(ConflictError):
            await svc.cancel(accepted.dispatch_id)
