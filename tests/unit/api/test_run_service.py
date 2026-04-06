"""Tests for RunService — guard error mapping, status completeness, ownership."""

from __future__ import annotations

from unittest.mock import AsyncMock

import pytest

from tanren_api.errors import ConflictError, NotFoundError
from tanren_api.models import ExecuteRequest
from tanren_api.services.run import RunService
from tanren_core.env.environment_schema import EnvironmentProfile
from tanren_core.schemas import AuthMode, Cli, Dispatch, Phase
from tanren_core.store.enums import DispatchMode, DispatchStatus, Lane, StepStatus, StepType
from tanren_core.store.views import DispatchView, StepView

DEFAULT_PROFILE = EnvironmentProfile(name="default")

# Minimal valid provision result JSON for tests
_PROV_RESULT = (
    '{"handle": {"env_id": "e1", "worktree_path": "/w", "branch": "main",'
    ' "project": "test", "provision_timestamp": "2026-01-01T00:00:00Z"}}'
)


def _dispatch_view(
    dispatch_id: str = "wf-test-1",
    status: DispatchStatus = DispatchStatus.RUNNING,
    user_id: str = "u-1",
) -> DispatchView:
    dispatch = Dispatch(
        workflow_id=dispatch_id,
        phase=Phase.DO_TASK,
        project="test",
        spec_folder=".",
        branch="main",
        cli=Cli.CLAUDE,
        auth=AuthMode.API_KEY,
        timeout=1800,
        resolved_profile=DEFAULT_PROFILE,
    )
    return DispatchView(
        dispatch_id=dispatch_id,
        mode=DispatchMode.MANUAL,
        status=status,
        outcome=None,
        lane=Lane.IMPL,
        preserve_on_failure=False,
        dispatch=dispatch,
        user_id=user_id,
        created_at="2026-01-01T00:00:00Z",
        updated_at="2026-01-01T00:00:00Z",
    )


def _step_view(
    step_type: StepType = StepType.PROVISION,
    status: StepStatus = StepStatus.COMPLETED,
    seq: int = 0,
    result_json: str | None = None,
) -> StepView:
    return StepView(
        step_id=f"s-{step_type}-{seq}",
        dispatch_id="wf-test-1",
        step_type=step_type,
        step_sequence=seq,
        lane=None,
        status=status,
        worker_id=None,
        result_json=result_json,
        error=None,
        retry_count=0,
        created_at="2026-01-01T00:00:00Z",
        updated_at="2026-01-01T00:00:00Z",
    )


def _service(
    state_store: AsyncMock | None = None,
) -> RunService:
    return RunService(
        event_store=AsyncMock(),
        job_queue=AsyncMock(),
        state_store=state_store or AsyncMock(),
    )


def _mock_state(
    dispatch: DispatchView | None = None,
    steps: list[StepView] | None = None,
) -> AsyncMock:
    """Build a mock StateStore with pre-configured return values."""
    store = AsyncMock()
    store.get_dispatch.return_value = dispatch
    store.get_steps_for_dispatch.return_value = steps or []
    return store


class TestExecuteGuardMapping:
    """Guard errors should map to 409 Conflict, not 500."""

    async def test_concurrent_execute_returns_409(self) -> None:
        view = _dispatch_view()
        state = _mock_state(
            dispatch=view,
            steps=[
                _step_view(
                    StepType.PROVISION,
                    StepStatus.COMPLETED,
                    result_json=_PROV_RESULT,
                ),
                _step_view(StepType.EXECUTE, StepStatus.RUNNING, seq=1),
            ],
        )
        svc = _service(state_store=state)
        body = ExecuteRequest(project="test", spec_path=".", phase=Phase.DO_TASK, cli=Cli.CLAUDE)

        with pytest.raises(ConflictError):
            await svc.execute("wf-test-1", body)

    async def test_post_teardown_execute_returns_409(self) -> None:
        view = _dispatch_view()
        state = _mock_state(
            dispatch=view,
            steps=[
                _step_view(
                    StepType.PROVISION,
                    StepStatus.COMPLETED,
                    result_json=_PROV_RESULT,
                ),
                _step_view(StepType.TEARDOWN, StepStatus.COMPLETED, seq=1),
            ],
        )
        svc = _service(state_store=state)
        body = ExecuteRequest(project="test", spec_path=".", phase=Phase.DO_TASK, cli=Cli.CLAUDE)

        with pytest.raises(ConflictError):
            await svc.execute("wf-test-1", body)


class TestStatusCompleteness:
    """run_status should populate all RunStatus fields."""

    async def test_status_includes_phase_and_timestamps(self) -> None:
        view = _dispatch_view()
        state = _mock_state(
            dispatch=view,
            steps=[_step_view(StepType.PROVISION, StepStatus.COMPLETED)],
        )
        svc = _service(state_store=state)

        result = await svc.status("wf-test-1")

        assert result.env_id == "wf-test-1"
        assert result.phase == Phase.DO_TASK
        assert result.started_at == "2026-01-01T00:00:00Z"
        assert result.duration_secs is not None
        assert result.duration_secs >= 0

    async def test_status_includes_vm_info_from_provision(self) -> None:
        import json

        prov_result = json.dumps({
            "handle": {
                "env_id": "e1",
                "worktree_path": "/w",
                "branch": "main",
                "project": "test",
                "provision_timestamp": "2026-01-01T00:00:00Z",
                "vm": {
                    "vm_id": "vm-1",
                    "host": "10.0.0.1",
                    "provider": "hetzner",
                    "created_at": "2026-01-01T00:00:00Z",
                },
            }
        })
        view = _dispatch_view()
        state = _mock_state(
            dispatch=view,
            steps=[_step_view(StepType.PROVISION, StepStatus.COMPLETED, result_json=prov_result)],
        )
        svc = _service(state_store=state)

        result = await svc.status("wf-test-1")

        assert result.vm_id == "vm-1"
        assert result.host == "10.0.0.1"


class TestStatusOwnership:
    """run_status should enforce ownership."""

    async def test_non_owner_gets_not_found(self) -> None:
        view = _dispatch_view(user_id="u-owner")
        state = _mock_state(dispatch=view)
        svc = _service(state_store=state)

        with pytest.raises(NotFoundError):
            await svc.status("wf-test-1", user_id="u-other", is_admin=False)

    async def test_admin_bypasses_ownership(self) -> None:
        view = _dispatch_view(user_id="u-owner")
        state = _mock_state(dispatch=view, steps=[])
        svc = _service(state_store=state)

        result = await svc.status("wf-test-1", user_id="u-other", is_admin=True)
        assert result.env_id == "wf-test-1"
