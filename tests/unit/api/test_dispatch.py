"""Tests for DispatchService."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any

import pytest

from tanren_api.models import DispatchRequest, DispatchRunStatus
from tanren_api.services.dispatch import DispatchService
from tanren_core.env.environment_schema import EnvironmentProfile
from tanren_core.schemas import AuthMode, Cli, Phase
from tanren_core.store.enums import DispatchStatus, StepStatus
from tanren_core.store.factory import create_store

if TYPE_CHECKING:
    from pathlib import Path

    from tanren_core.store.repository import Store

DEFAULT_PROFILE = EnvironmentProfile(name="default")


@pytest.fixture
async def store(tmp_path: Path):
    s = await create_store(str(tmp_path / "test.db"))
    yield s
    await s.close()


def _make_request(**overrides: Any) -> DispatchRequest:
    defaults: dict[str, Any] = {
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


class TestDispatchServiceCreate:
    async def test_create_returns_dispatch_id(self, store: Store) -> None:
        svc = DispatchService(event_store=store, job_queue=store, state_store=store)
        result = await svc.create(_make_request())
        assert result.dispatch_id.startswith("wf-test-")

    async def test_create_enqueues_provision_step(self, store: Store) -> None:
        svc = DispatchService(event_store=store, job_queue=store, state_store=store)
        accepted = await svc.create(_make_request())

        steps = await store.get_steps_for_dispatch(accepted.dispatch_id)
        assert len(steps) == 1
        assert steps[0].step_type.value == "provision"

    async def test_create_sets_dispatch_to_running(self, store: Store) -> None:
        svc = DispatchService(event_store=store, job_queue=store, state_store=store)
        accepted = await svc.create(_make_request())

        view = await store.get_dispatch(accepted.dispatch_id)
        assert view is not None
        assert view.status == DispatchStatus.RUNNING

    async def test_create_appends_dispatch_created_event(self, store: Store) -> None:
        svc = DispatchService(event_store=store, job_queue=store, state_store=store)
        accepted = await svc.create(_make_request())

        events = await store.query_events(entity_id=accepted.dispatch_id)
        types = [e.event_type for e in events.events]
        assert "DispatchCreated" in types
        assert "StepEnqueued" in types


class TestDispatchServiceGet:
    async def test_get_returns_dispatch_detail(self, store: Store) -> None:
        svc = DispatchService(event_store=store, job_queue=store, state_store=store)
        accepted = await svc.create(_make_request())

        detail = await svc.get(accepted.dispatch_id)
        assert detail.workflow_id == accepted.dispatch_id
        assert detail.project == "test"
        assert detail.status == DispatchRunStatus.RUNNING

    async def test_get_not_found(self, store: Store) -> None:
        svc = DispatchService(event_store=store, job_queue=store, state_store=store)
        from tanren_api.errors import NotFoundError

        with pytest.raises(NotFoundError):
            await svc.get("nonexistent")


class TestDispatchServiceCancel:
    async def test_cancel_pending_dispatch(self, store: Store) -> None:
        svc = DispatchService(event_store=store, job_queue=store, state_store=store)

        # Create a dispatch and manually set it back to pending
        accepted = await svc.create(_make_request())
        await store.update_dispatch_status(accepted.dispatch_id, DispatchStatus.PENDING)

        cancelled = await svc.cancel(accepted.dispatch_id)
        assert cancelled.dispatch_id == accepted.dispatch_id

        view = await store.get_dispatch(accepted.dispatch_id)
        assert view is not None
        assert view.status == DispatchStatus.CANCELLED

    async def test_cancel_also_cancels_pending_steps(self, store: Store) -> None:
        svc = DispatchService(event_store=store, job_queue=store, state_store=store)

        accepted = await svc.create(_make_request())
        await store.update_dispatch_status(accepted.dispatch_id, DispatchStatus.PENDING)

        # Verify there's a pending step (provision)
        steps_before = await store.get_steps_for_dispatch(accepted.dispatch_id)
        pending = [s for s in steps_before if s.status == StepStatus.PENDING]
        assert len(pending) >= 1

        await svc.cancel(accepted.dispatch_id)

        # All pending steps should now be cancelled
        steps_after = await store.get_steps_for_dispatch(accepted.dispatch_id)
        still_pending = [s for s in steps_after if s.status == StepStatus.PENDING]
        assert still_pending == []

    async def test_cancel_completed_dispatch_raises_conflict(self, store: Store) -> None:
        from tanren_api.errors import ConflictError
        from tanren_core.schemas import Outcome

        svc = DispatchService(event_store=store, job_queue=store, state_store=store)
        accepted = await svc.create(_make_request())
        await store.update_dispatch_status(
            accepted.dispatch_id,
            DispatchStatus.COMPLETED,
            Outcome.SUCCESS,
        )

        with pytest.raises(ConflictError):
            await svc.cancel(accepted.dispatch_id)
