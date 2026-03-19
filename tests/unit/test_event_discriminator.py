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

_TS = "2026-01-01T00:00:00Z"
_WF = "wf-proj-1-1234"

EVENT_INSTANCES = [
    DispatchReceived(timestamp=_TS, workflow_id=_WF, phase="do-task", project="proj", cli="claude"),
    PhaseStarted(timestamp=_TS, workflow_id=_WF, phase="do-task", worktree_path="/tmp/wt"),
    PhaseCompleted(
        timestamp=_TS,
        workflow_id=_WF,
        phase="do-task",
        project="proj",
        outcome="success",
        duration_secs=10,
        exit_code=0,
    ),
    PreflightCompleted(timestamp=_TS, workflow_id=_WF, passed=True),
    PostflightCompleted(timestamp=_TS, workflow_id=_WF, phase="do-task"),
    ErrorOccurred(timestamp=_TS, workflow_id=_WF, phase="do-task", error="boom"),
    RetryScheduled(
        timestamp=_TS, workflow_id=_WF, phase="do-task", attempt=1, max_attempts=3, backoff_secs=5
    ),
    VMProvisioned(
        timestamp=_TS,
        workflow_id=_WF,
        vm_id="vm-1",
        host="10.0.0.1",
        provider=VMProvider.HETZNER,
        project="proj",
        profile="default",
    ),
    VMReleased(timestamp=_TS, workflow_id=_WF, vm_id="vm-1", project="proj", duration_secs=600),
    BootstrapCompleted(timestamp=_TS, workflow_id=_WF, vm_id="vm-1"),
    TokenUsageRecorded(
        timestamp=_TS,
        workflow_id=_WF,
        phase="do-task",
        project="proj",
        cli="claude",
        input_tokens=1000,
        output_tokens=500,
        total_tokens=1500,
        total_cost=0.05,
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
