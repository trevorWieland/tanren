"""Lifecycle events for the event-sourced dispatch system.

These events drive state transitions and are the source of truth for all
dispatch and step state.  They live alongside existing observability events
(``VMProvisioned``, ``TokenUsageRecorded``, etc.) in the same unified
``events`` table.
"""

from __future__ import annotations

from typing import Literal

from pydantic import BaseModel, ConfigDict, Field, model_validator

from tanren_core.adapters.events import Event
from tanren_core.schemas import Dispatch, Outcome
from tanren_core.store.enums import DispatchMode, Lane, StepType
from tanren_core.store.payloads import DryRunResult, ExecuteResult, ProvisionResult, TeardownResult

# ── Dispatch-level events ─────────────────────────────────────────────────


class DispatchCreated(Event):
    """A new dispatch was accepted and its first step enqueued."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    type: Literal["dispatch_created"] = Field(
        default="dispatch_created",
        description="Event type discriminator",
    )
    dispatch: Dispatch = Field(
        ...,
        description="Full dispatch payload (immutable snapshot)",
    )
    mode: DispatchMode = Field(..., description="auto or manual step management")
    lane: Lane = Field(..., description="Concurrency lane for the execute step")


class DispatchCompleted(Event):
    """All steps succeeded — the dispatch is done."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    type: Literal["dispatch_completed"] = Field(
        default="dispatch_completed",
        description="Event type discriminator",
    )
    outcome: Outcome = Field(..., description="Final dispatch outcome")
    total_duration_secs: int = Field(
        ...,
        ge=0,
        description="Total wall-clock time from creation to completion",
    )


class DispatchFailed(Event):
    """The dispatch failed terminally."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    type: Literal["dispatch_failed"] = Field(
        default="dispatch_failed",
        description="Event type discriminator",
    )
    outcome: Outcome = Field(default=Outcome.ERROR, description="Final dispatch outcome")
    failed_step_id: str = Field(..., description="The step that caused the failure")
    failed_step_type: StepType = Field(..., description="Type of the failed step")
    error: str = Field(..., description="Terminal error message")


# ── Step-level events ─────────────────────────────────────────────────────


class StepEnqueued(Event):
    """A step was added to the job queue."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    type: Literal["step_enqueued"] = Field(
        default="step_enqueued",
        description="Event type discriminator",
    )
    step_id: str = Field(..., description="Unique step identifier (UUID)")
    step_type: StepType = Field(..., description="provision, execute, or teardown")
    step_sequence: int = Field(
        ...,
        ge=0,
        description="Ordering index within the dispatch (0=provision, 1=execute, 2=teardown)",
    )
    lane: Lane | None = Field(
        default=None,
        description="Concurrency lane (set for execute steps, None for provision/teardown)",
    )


class StepDequeued(Event):
    """A worker claimed a step from the queue."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    type: Literal["step_dequeued"] = Field(
        default="step_dequeued",
        description="Event type discriminator",
    )
    step_id: str = Field(..., description="Step that was claimed")
    worker_id: str = Field(..., description="Identifier of the claiming worker")


class StepStarted(Event):
    """A worker began executing a step."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    type: Literal["step_started"] = Field(
        default="step_started",
        description="Event type discriminator",
    )
    step_id: str = Field(..., description="Step that started")
    worker_id: str = Field(..., description="Worker executing the step")
    step_type: StepType = Field(..., description="Step type for quick filtering")


# ── Step result payload (discriminated union by step_type) ────────────────

StepResultPayload = ProvisionResult | ExecuteResult | TeardownResult | DryRunResult

_STEP_RESULT_MODEL: dict[str, type[BaseModel]] = {
    StepType.PROVISION: ProvisionResult,
    StepType.EXECUTE: ExecuteResult,
    StepType.TEARDOWN: TeardownResult,
    StepType.DRY_RUN: DryRunResult,
}


class StepCompleted(Event):
    """A step finished successfully."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    type: Literal["step_completed"] = Field(
        default="step_completed",
        description="Event type discriminator",
    )
    step_id: str = Field(..., description="Step that completed")
    step_type: StepType = Field(..., description="Step type for quick filtering")
    duration_secs: int = Field(..., ge=0, description="Wall-clock step duration")
    result_payload: ProvisionResult | ExecuteResult | TeardownResult | DryRunResult = Field(
        ...,
        description="Step-type-specific result data",
    )

    @model_validator(mode="before")
    @classmethod
    def _resolve_result_payload(cls, data: dict[str, object]) -> dict[str, object]:
        # Deserialize result_payload using step_type as discriminator.
        if not isinstance(data, dict):
            return data
        step_type = data.get("step_type")
        raw = data.get("result_payload")
        if isinstance(raw, dict) and isinstance(step_type, str):
            model_cls = _STEP_RESULT_MODEL.get(step_type)
            if model_cls is not None:
                data = dict(data)
                data["result_payload"] = model_cls.model_validate(raw)
        return data


class StepFailed(Event):
    """A step failed."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    type: Literal["step_failed"] = Field(
        default="step_failed",
        description="Event type discriminator",
    )
    step_id: str = Field(..., description="Step that failed")
    step_type: StepType = Field(..., description="Step type for quick filtering")
    error: str = Field(..., description="Error message")
    error_class: str | None = Field(
        default=None,
        description="Classified error type (transient, fatal, ambiguous)",
    )
    retry_count: int = Field(default=0, ge=0, description="Times this step has been retried")
    duration_secs: int = Field(..., ge=0, description="Wall-clock duration before failure")
