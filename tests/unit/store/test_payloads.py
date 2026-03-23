"""Tests for step payload and result models."""

from typing import Any

import pytest
from pydantic import ValidationError

from tanren_core.env.environment_schema import EnvironmentProfile
from tanren_core.schemas import AuthMode, Cli, Dispatch, Finding, FindingSeverity, Outcome, Phase
from tanren_core.store.handle import PersistedEnvironmentHandle
from tanren_core.store.payloads import (
    ExecuteResult,
    ExecuteStepPayload,
    ProvisionResult,
    ProvisionStepPayload,
    TeardownResult,
    TeardownStepPayload,
)

DEFAULT_PROFILE = EnvironmentProfile(name="default")


def _make_dispatch(**overrides: Any) -> Dispatch:
    defaults: dict[str, Any] = {
        "workflow_id": "wf-test-1-100",
        "phase": Phase.DO_TASK,
        "project": "test",
        "spec_folder": "spec/001",
        "branch": "main",
        "cli": Cli.CLAUDE,
        "auth": AuthMode.API_KEY,
        "timeout": 1800,
        "resolved_profile": DEFAULT_PROFILE,
    }
    return Dispatch.model_validate(defaults | overrides)


def _make_handle(**overrides: Any) -> PersistedEnvironmentHandle:
    defaults: dict[str, Any] = {
        "env_id": "env-abc",
        "worktree_path": "/workspace/test",
        "branch": "main",
        "project": "test",
        "provision_timestamp": "2026-01-01T00:00:00Z",
    }
    return PersistedEnvironmentHandle.model_validate(defaults | overrides)


class TestProvisionStepPayload:
    def test_roundtrip(self) -> None:
        payload = ProvisionStepPayload(dispatch=_make_dispatch())
        restored = ProvisionStepPayload.model_validate_json(payload.model_dump_json())
        assert restored.dispatch.workflow_id == "wf-test-1-100"

    def test_dispatch_required(self) -> None:
        with pytest.raises(ValidationError):
            ProvisionStepPayload()


class TestExecuteStepPayload:
    def test_roundtrip(self) -> None:
        payload = ExecuteStepPayload(dispatch=_make_dispatch(), handle=_make_handle())
        restored = ExecuteStepPayload.model_validate_json(payload.model_dump_json())
        assert restored.handle.env_id == "env-abc"


class TestTeardownStepPayload:
    def test_preserve_default_false(self) -> None:
        payload = TeardownStepPayload(dispatch=_make_dispatch(), handle=_make_handle())
        assert payload.preserve is False

    def test_preserve_true(self) -> None:
        payload = TeardownStepPayload(
            dispatch=_make_dispatch(), handle=_make_handle(), preserve=True
        )
        assert payload.preserve is True


class TestProvisionResult:
    def test_roundtrip(self) -> None:
        result = ProvisionResult(handle=_make_handle())
        restored = ProvisionResult.model_validate_json(result.model_dump_json())
        assert restored.handle.project == "test"


class TestExecuteResult:
    def test_minimal(self) -> None:
        result = ExecuteResult(
            outcome=Outcome.SUCCESS,
            exit_code=0,
            duration_secs=120,
        )
        assert result.signal is None
        assert result.findings == []
        assert result.token_usage is None

    def test_with_findings(self) -> None:
        result = ExecuteResult(
            outcome=Outcome.FAIL,
            exit_code=1,
            duration_secs=60,
            findings=[Finding(title="Bug", severity=FindingSeverity.FIX)],
            new_tasks=[Finding(title="Fix bug", severity=FindingSeverity.FIX)],
        )
        assert len(result.findings) == 1
        assert result.findings[0].title == "Bug"
        assert len(result.new_tasks) == 1

    def test_roundtrip(self) -> None:
        result = ExecuteResult(
            outcome=Outcome.SUCCESS,
            exit_code=0,
            duration_secs=10,
            pushed=True,
            spec_modified=False,
        )
        restored = ExecuteResult.model_validate_json(result.model_dump_json())
        assert restored == result


class TestTeardownResult:
    def test_defaults(self) -> None:
        result = TeardownResult()
        assert result.vm_released is True
        assert result.duration_secs == 0
        assert result.estimated_cost is None

    def test_with_cost(self) -> None:
        result = TeardownResult(vm_released=True, duration_secs=5, estimated_cost=0.15)
        assert result.estimated_cost == pytest.approx(0.15)
