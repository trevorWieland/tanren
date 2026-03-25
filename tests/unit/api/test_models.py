"""Tests for API request/response model validation."""

import pytest
from pydantic import ValidationError

from tanren_api.models import (
    ConfigResponse,
    DispatchAccepted,
    DispatchCancelled,
    DispatchDetail,
    DispatchRequest,
    DispatchRunStatus,
    ErrorResponse,
    ExecuteRequest,
    HealthResponse,
    PaginatedEvents,
    ProvisionRequest,
    ReadinessResponse,
    RunEnvironment,
    RunEnvironmentStatus,
    RunExecuteAccepted,
    RunFullRequest,
    RunStatus,
    RunTeardownAccepted,
    VMDryRunResult,
    VMReleaseConfirmed,
    VMStatus,
    VMSummary,
)
from tanren_core.adapters.remote_types import VMProvider, VMRequirements
from tanren_core.env.environment_schema import EnvironmentProfile
from tanren_core.roles import AuthMode
from tanren_core.schemas import Cli, Outcome, Phase

DEFAULT_PROFILE = EnvironmentProfile(name="default")


@pytest.mark.api
class TestModels:
    def test_dispatch_request_validates(self):
        req = DispatchRequest(
            project="my-project",
            phase=Phase.DO_TASK,
            branch="main",
            spec_folder="specs/feature",
            cli=Cli.CLAUDE,
            resolved_profile=DEFAULT_PROFILE,
        )
        assert req.project == "my-project"
        assert req.timeout == 1800

    def test_dispatch_request_rejects_extra_fields(self):
        with pytest.raises(ValidationError):
            DispatchRequest.model_validate({
                "project": "p",
                "phase": Phase.DO_TASK,
                "branch": "main",
                "spec_folder": "s",
                "cli": Cli.CLAUDE,
                "resolved_profile": DEFAULT_PROFILE.model_dump(),
                "unknown_field": "x",
            })

    def test_dispatch_request_timeout_minimum(self):
        with pytest.raises(ValidationError):
            DispatchRequest(
                project="p",
                phase=Phase.DO_TASK,
                branch="main",
                spec_folder="s",
                cli=Cli.CLAUDE,
                resolved_profile=DEFAULT_PROFILE,
                timeout=0,
            )

    def test_provision_request_validates(self):
        req = ProvisionRequest(project="proj", branch="dev", resolved_profile=DEFAULT_PROFILE)
        assert req.environment_profile == "default"

    def test_execute_request_validates(self):
        req = ExecuteRequest(
            project="proj",
            spec_path="specs/x",
            phase=Phase.DO_TASK,
            cli=Cli.CLAUDE,
            auth=AuthMode.API_KEY,
        )
        assert req.timeout == 1800
        assert req.cli == Cli.CLAUDE

    def test_execute_request_rejects_extra_fields(self):
        with pytest.raises(ValidationError):
            ExecuteRequest.model_validate({
                "project": "p",
                "spec_path": "s",
                "phase": Phase.DO_TASK,
                "cli": "claude",
                "auth": "api_key",
                "unknown_field": "x",
            })

    def test_execute_request_timeout_minimum(self):
        with pytest.raises(ValidationError):
            ExecuteRequest(
                project="p",
                spec_path="s",
                phase=Phase.DO_TASK,
                cli=Cli.CLAUDE,
                auth=AuthMode.API_KEY,
                timeout=0,
            )

    def test_run_full_request_validates(self):
        from tanren_core.env.environment_schema import EnvironmentProfile

        req = RunFullRequest(
            project="proj",
            branch="main",
            spec_path="specs/x",
            phase=Phase.GATE,
            cli=Cli.CLAUDE,
            auth=AuthMode.API_KEY,
            resolved_profile=EnvironmentProfile(name="default"),
        )
        assert req.timeout == 1800

    def test_dispatch_accepted_serialization(self):
        resp = DispatchAccepted(dispatch_id="abc-123")
        data = resp.model_dump()
        assert data["dispatch_id"] == "abc-123"
        assert data["status"] == "accepted"

    def test_health_response_serialization(self):
        resp = HealthResponse(status="ok", version="0.1.0", uptime_seconds=42.5)
        data = resp.model_dump()
        assert data["status"] == "ok"
        assert "uptime_seconds" in data

    def test_error_response_serialization(self):
        resp = ErrorResponse(
            detail="Not found",
            error_code="not_found",
            timestamp="2026-01-01T00:00:00Z",
        )
        data = resp.model_dump()
        assert data["error_code"] == "not_found"
        assert data["request_id"] is None

    def test_config_response_validates(self):
        resp = ConfigResponse(
            db_backend="sqlite",
            store_connected=True,
            worker_lanes={"impl": 1, "audit": 1, "gate": 3, "provision": 10},
            remote_enabled=True,
            version="0.1.0",
        )
        assert resp.store_connected is True

    def test_paginated_events_validates(self):
        resp = PaginatedEvents(events=[], total=0, limit=50, offset=0)
        assert resp.total == 0

    def test_readiness_response(self):
        resp = ReadinessResponse(status="ready")
        assert resp.status == "ready"

    def test_readiness_response_rejects_extra(self):
        with pytest.raises(ValidationError):
            ReadinessResponse.model_validate({"status": "ready", "extra": "bad"})

    def test_dispatch_detail_validates(self):
        detail = DispatchDetail(
            workflow_id="wf-proj-1-1234",
            phase=Phase.DO_TASK,
            project="proj",
            spec_folder="specs/a",
            branch="main",
            cli=Cli.CLAUDE,
            timeout=1800,
            environment_profile="default",
            status=DispatchRunStatus.RUNNING,
            created_at="2026-01-01T00:00:00Z",
        )
        assert detail.status == DispatchRunStatus.RUNNING
        assert detail.outcome is None

    def test_dispatch_detail_with_outcome(self):
        detail = DispatchDetail(
            workflow_id="wf-proj-1-1234",
            phase=Phase.DO_TASK,
            project="proj",
            spec_folder="specs/a",
            branch="main",
            cli=Cli.CLAUDE,
            timeout=1800,
            environment_profile="default",
            status=DispatchRunStatus.COMPLETED,
            outcome=Outcome.SUCCESS,
            created_at="2026-01-01T00:00:00Z",
            started_at="2026-01-01T00:00:01Z",
            completed_at="2026-01-01T00:01:00Z",
        )
        assert detail.outcome == Outcome.SUCCESS

    def test_dispatch_cancelled_defaults(self):
        resp = DispatchCancelled(dispatch_id="wf-123")
        assert resp.status == DispatchRunStatus.CANCELLED

    def test_vm_summary_validates(self):
        vm = VMSummary(
            vm_id="vm-1",
            host="10.0.0.1",
            provider=VMProvider.HETZNER,
            status=VMStatus.ACTIVE,
            created_at="2026-01-01T00:00:00Z",
        )
        assert vm.workflow_id is None
        assert vm.project is None

    def test_vm_release_confirmed_defaults(self):
        resp = VMReleaseConfirmed(vm_id="vm-1")
        assert resp.status == VMStatus.RELEASED

    def test_vm_dry_run_result_validates(self):
        result = VMDryRunResult(
            provider=VMProvider.HETZNER,
            server_type="cx21",
            estimated_cost_hourly=0.05,
            would_provision=True,
            requirements=VMRequirements(profile="default"),
        )
        assert result.would_provision is True
        assert result.requirements.profile == "default"

    def test_vm_dry_run_rejects_negative_cost(self):
        with pytest.raises(ValidationError):
            VMDryRunResult(
                provider=VMProvider.HETZNER,
                estimated_cost_hourly=-1.0,
                would_provision=True,
                requirements=VMRequirements(profile="default"),
            )

    def test_run_environment_defaults(self):
        env = RunEnvironment(env_id="env-1", vm_id="vm-1", host="10.0.0.1")
        assert env.status == RunEnvironmentStatus.PROVISIONED

    def test_run_execute_accepted_defaults(self):
        resp = RunExecuteAccepted(env_id="env-1", dispatch_id="wf-123")
        assert resp.status == RunEnvironmentStatus.EXECUTING

    def test_run_teardown_accepted_defaults(self):
        resp = RunTeardownAccepted(env_id="env-1")
        assert resp.status == RunEnvironmentStatus.TEARING_DOWN

    def test_run_status_validates(self):
        status = RunStatus(
            env_id="env-1",
            status=RunEnvironmentStatus.EXECUTING,
            phase=Phase.DO_TASK,
            started_at="2026-01-01T00:00:00Z",
            duration_secs=120,
        )
        assert status.outcome is None

    def test_run_status_rejects_negative_duration(self):
        with pytest.raises(ValidationError):
            RunStatus(
                env_id="env-1",
                status=RunEnvironmentStatus.EXECUTING,
                duration_secs=-1,
            )

    def test_dispatch_run_status_values(self):
        assert DispatchRunStatus.PENDING == "pending"
        assert DispatchRunStatus.CANCELLED == "cancelled"
        assert len(DispatchRunStatus) == 5

    def test_vm_status_values(self):
        assert VMStatus.ACTIVE == "active"
        assert VMStatus.RELEASED == "released"
        assert VMStatus.FAILED == "failed"
        assert len(VMStatus) == 5

    def test_run_environment_status_values(self):
        assert RunEnvironmentStatus.PROVISIONING == "provisioning"
        assert RunEnvironmentStatus.FAILED == "failed"
        assert len(RunEnvironmentStatus) == 6

    def test_all_models_have_field_descriptions(self):
        for model_cls in [
            DispatchRequest,
            ProvisionRequest,
            ExecuteRequest,
            RunFullRequest,
            DispatchAccepted,
            HealthResponse,
            ErrorResponse,
            ConfigResponse,
            PaginatedEvents,
            ReadinessResponse,
            DispatchDetail,
            DispatchCancelled,
            VMSummary,
            VMReleaseConfirmed,
            VMDryRunResult,
            RunEnvironment,
            RunExecuteAccepted,
            RunTeardownAccepted,
            RunStatus,
        ]:
            for name, field in model_cls.model_fields.items():
                assert field.description, f"{model_cls.__name__}.{name} missing description"
