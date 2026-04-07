"""Tests for event type discriminator and EventPayload union."""

import pytest
from pydantic import TypeAdapter

from tanren_api.models import EventPayload, PaginatedEvents
from tanren_core.adapters.events import (
    BootstrapCompleted,
    DispatchReceived,
    ErrorOccurred,
    PhaseCompleted,
    PhaseStarted,
    PostflightCompleted,
    PreflightCompleted,
    RetryScheduled,
    TokenUsageRecorded,
    VMProvisioned,
    VMReleased,
)
from tanren_core.adapters.remote_types import VMProvider
from tanren_core.env.environment_schema import EnvironmentProfile
from tanren_core.schemas import Cli, Dispatch, Outcome, Phase
from tanren_core.store.auth_events import (
    KeyCreated,
    KeyRevoked,
    KeyRotated,
    UserCreated,
    UserDeactivated,
    UserUpdated,
)
from tanren_core.store.enums import DispatchMode, Lane, StepType
from tanren_core.store.events import (
    DispatchCompleted,
    DispatchCreated,
    DispatchFailed,
    StepCompleted,
    StepEnqueued,
    StepFailed,
    StepStarted,
)
from tanren_core.store.payloads import TeardownResult

_TS = "2026-01-01T00:00:00Z"
_WF = "wf-proj-1-1234"

EVENT_INSTANCES = [
    DispatchReceived(timestamp=_TS, entity_id=_WF, phase="do-task", project="proj", cli="claude"),
    PhaseStarted(timestamp=_TS, entity_id=_WF, phase="do-task", worktree_path="/tmp/wt"),
    PhaseCompleted(
        timestamp=_TS,
        entity_id=_WF,
        phase="do-task",
        project="proj",
        outcome="success",
        duration_secs=10,
        exit_code=0,
    ),
    PreflightCompleted(timestamp=_TS, entity_id=_WF, passed=True),
    PostflightCompleted(timestamp=_TS, entity_id=_WF, phase="do-task"),
    ErrorOccurred(timestamp=_TS, entity_id=_WF, phase="do-task", error="boom"),
    RetryScheduled(
        timestamp=_TS, entity_id=_WF, phase="do-task", attempt=1, max_attempts=3, backoff_secs=5
    ),
    VMProvisioned(
        timestamp=_TS,
        entity_id=_WF,
        vm_id="vm-1",
        host="10.0.0.1",
        provider=VMProvider.HETZNER,
        project="proj",
        profile="default",
    ),
    VMReleased(timestamp=_TS, entity_id=_WF, vm_id="vm-1", project="proj", duration_secs=600),
    BootstrapCompleted(timestamp=_TS, entity_id=_WF, vm_id="vm-1"),
    TokenUsageRecorded(
        timestamp=_TS,
        entity_id=_WF,
        phase="do-task",
        project="proj",
        cli="claude",
        input_tokens=1000,
        output_tokens=500,
        total_tokens=1500,
        total_cost=0.05,
    ),
    # Store lifecycle events
    DispatchCreated(
        timestamp=_TS,
        entity_id=_WF,
        dispatch=Dispatch(
            workflow_id=_WF,
            phase=Phase.DO_TASK,
            project="proj",
            spec_folder="specs/001",
            branch="main",
            cli=Cli.CLAUDE,
            timeout=1800,
            resolved_profile=EnvironmentProfile(name="default"),
        ),
        mode=DispatchMode.AUTO,
        lane=Lane.IMPL,
    ),
    DispatchCompleted(
        timestamp=_TS, entity_id=_WF, outcome=Outcome.SUCCESS, total_duration_secs=60
    ),
    DispatchFailed(
        timestamp=_TS,
        entity_id=_WF,
        failed_step_id="step-1",
        failed_step_type=StepType.EXECUTE,
        error="boom",
    ),
    StepEnqueued(
        timestamp=_TS,
        entity_id=_WF,
        step_id="step-1",
        step_type=StepType.PROVISION,
        step_sequence=0,
    ),
    StepStarted(
        timestamp=_TS,
        entity_id=_WF,
        step_id="step-1",
        worker_id="w1",
        step_type=StepType.PROVISION,
    ),
    StepCompleted(
        timestamp=_TS,
        entity_id=_WF,
        step_id="step-1",
        step_type=StepType.TEARDOWN,
        duration_secs=5,
        result_payload=TeardownResult(vm_released=True, duration_secs=5),
    ),
    StepFailed(
        timestamp=_TS,
        entity_id=_WF,
        step_id="step-1",
        step_type=StepType.EXECUTE,
        error="timeout",
        duration_secs=30,
    ),
    # Auth lifecycle events
    UserCreated(
        timestamp=_TS,
        entity_id="user-1",
        entity_type="user",
        user_id="user-1",
        name="Alice",
        email="alice@example.com",
        role="admin",
    ),
    UserUpdated(
        timestamp=_TS,
        entity_id="user-1",
        entity_type="user",
        name="Alice B.",
    ),
    UserDeactivated(
        timestamp=_TS,
        entity_id="user-1",
        entity_type="user",
    ),
    KeyCreated(
        timestamp=_TS,
        entity_id="key-1",
        entity_type="api_key",
        key_id="key-1",
        user_id="user-1",
        name="dev-key",
        key_prefix="tnrn1234",
        scopes=["dispatch:create", "dispatch:read"],
    ),
    KeyRevoked(
        timestamp=_TS,
        entity_id="key-1",
        entity_type="api_key",
    ),
    KeyRotated(
        timestamp=_TS,
        entity_id="key-1",
        entity_type="api_key",
        new_key_id="key-2",
        grace_expires_at="2026-01-02T00:00:00Z",
    ),
]

EXPECTED_TYPES = [
    "dispatch_received",
    "phase_started",
    "phase_completed",
    "preflight_completed",
    "postflight_completed",
    "error_occurred",
    "retry_scheduled",
    "vm_provisioned",
    "vm_released",
    "bootstrap_completed",
    "token_usage_recorded",
    "dispatch_created",
    "dispatch_completed",
    "dispatch_failed",
    "step_enqueued",
    "step_started",
    "step_completed",
    "step_failed",
    "user_created",
    "user_updated",
    "user_deactivated",
    "key_created",
    "key_revoked",
    "key_rotated",
]


@pytest.mark.api
class TestEventDiscriminator:
    @pytest.mark.parametrize(
        ("event", "expected_type"),
        list(zip(EVENT_INSTANCES, EXPECTED_TYPES, strict=True)),
        ids=EXPECTED_TYPES,
    )
    def test_event_serializes_with_type(self, event, expected_type):
        data = event.model_dump()
        assert data["type"] == expected_type

    @pytest.mark.parametrize(
        ("event", "expected_type"),
        list(zip(EVENT_INSTANCES, EXPECTED_TYPES, strict=True)),
        ids=EXPECTED_TYPES,
    )
    def test_discriminated_union_round_trip(self, event, expected_type):
        adapter = TypeAdapter(EventPayload)
        data = event.model_dump()
        restored = adapter.validate_python(data)
        assert type(restored) is type(event)
        assert restored.model_dump() == data

    def test_paginated_events_accepts_typed_events(self):
        page = PaginatedEvents(
            events=EVENT_INSTANCES,
            total=len(EVENT_INSTANCES),
            limit=50,
            offset=0,
        )
        assert len(page.events) == len(EVENT_INSTANCES)
        for event, expected_type in zip(page.events, EXPECTED_TYPES, strict=True):
            assert event.model_dump()["type"] == expected_type

    def test_paginated_events_round_trip(self):
        page = PaginatedEvents(
            events=EVENT_INSTANCES,
            total=len(EVENT_INSTANCES),
            limit=50,
            offset=0,
        )
        data = page.model_dump()
        restored = PaginatedEvents.model_validate(data)
        assert len(restored.events) == len(EVENT_INSTANCES)
        for original, restored_event in zip(EVENT_INSTANCES, restored.events, strict=True):
            assert type(restored_event) is type(original)
