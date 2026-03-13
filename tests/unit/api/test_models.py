"""Tests for API request/response model validation."""

import pytest
from pydantic import ValidationError

from tanren_api.models import (
    ConfigResponse,
    DispatchAccepted,
    DispatchRequest,
    ErrorResponse,
    HealthResponse,
    PaginatedEvents,
    ProvisionRequest,
    RunFullRequest,
)
from tanren_core.schemas import Cli, Phase


@pytest.mark.api
class TestModels:
    def test_dispatch_request_validates(self):
        req = DispatchRequest(
            project="my-project",
            phase=Phase.DO_TASK,
            branch="main",
            spec_folder="specs/feature",
            cli=Cli.CLAUDE,
        )
        assert req.project == "my-project"
        assert req.timeout == 1800

    def test_dispatch_request_rejects_extra_fields(self):
        with pytest.raises(ValidationError):
            DispatchRequest(
                project="p",
                phase=Phase.DO_TASK,
                branch="main",
                spec_folder="s",
                cli=Cli.CLAUDE,
                unknown_field="x",
            )

    def test_dispatch_request_timeout_minimum(self):
        with pytest.raises(ValidationError):
            DispatchRequest(
                project="p",
                phase=Phase.DO_TASK,
                branch="main",
                spec_folder="s",
                cli=Cli.CLAUDE,
                timeout=0,
            )

    def test_provision_request_validates(self):
        req = ProvisionRequest(project="proj", branch="dev")
        assert req.environment_profile == "default"

    def test_run_full_request_validates(self):
        req = RunFullRequest(
            project="proj",
            branch="main",
            spec_path="specs/x",
            phase=Phase.GATE,
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
            ipc_dir="/tmp/ipc",
            github_dir="/tmp/github",
            poll_interval=5.0,
            heartbeat_interval=30.0,
            max_opencode=2,
            max_codex=2,
            max_gate=1,
            events_enabled=True,
            remote_enabled=False,
        )
        assert resp.events_enabled is True

    def test_paginated_events_validates(self):
        resp = PaginatedEvents(events=[], total=0, limit=50, offset=0)
        assert resp.total == 0

    def test_all_models_have_field_descriptions(self):
        for model_cls in [
            DispatchRequest,
            ProvisionRequest,
            RunFullRequest,
            DispatchAccepted,
            HealthResponse,
            ErrorResponse,
            ConfigResponse,
            PaginatedEvents,
        ]:
            for name, field in model_cls.model_fields.items():
                assert field.description, f"{model_cls.__name__}.{name} missing description"
