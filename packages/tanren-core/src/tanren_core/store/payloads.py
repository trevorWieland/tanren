"""Step payloads and result models for the dispatch job queue."""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict, Field

from tanren_core.ccusage import TokenUsage
from tanren_core.postflight import IntegrityRepairs
from tanren_core.schemas import Dispatch, Finding, Outcome
from tanren_core.store.handle import PersistedEnvironmentHandle

# ── Step payloads (what the worker receives) ──────────────────────────────


class ProvisionStepPayload(BaseModel):
    """Data needed to execute a provision step."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    dispatch: Dispatch = Field(
        ...,
        description="Full dispatch payload (carries resolved_profile)",
    )


class ExecuteStepPayload(BaseModel):
    """Data needed to execute an execute step."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    dispatch: Dispatch = Field(..., description="Full dispatch payload")
    handle: PersistedEnvironmentHandle = Field(
        ...,
        description="Serialized environment handle from provision step",
    )


class TeardownStepPayload(BaseModel):
    """Data needed to execute a teardown step."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    dispatch: Dispatch = Field(
        ...,
        description="Full dispatch payload (for workflow_id, project reference)",
    )
    handle: PersistedEnvironmentHandle = Field(
        ...,
        description="Serialized environment handle to tear down",
    )
    preserve: bool = Field(
        default=False,
        description="If True, skip actual teardown (preserve_on_failure triggered)",
    )


# ── Step results (what the worker produces) ───────────────────────────────


class ProvisionResult(BaseModel):
    """Result data from a successful provision step."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    handle: PersistedEnvironmentHandle = Field(
        ...,
        description="Serialized environment handle for subsequent steps",
    )


class ExecuteResult(BaseModel):
    """Result data from a successful execute step."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    outcome: Outcome = Field(..., description="Phase execution outcome")
    signal: str | None = Field(default=None, description="Agent signal file content")
    exit_code: int = Field(..., description="Process exit code")
    duration_secs: int = Field(..., ge=0, description="Phase execution duration")
    gate_output: str | None = Field(
        default=None,
        description="Gate output (gate phases only)",
    )
    tail_output: str | None = Field(
        default=None,
        description="Last 200 lines of stdout",
    )
    stderr_tail: str | None = Field(
        default=None,
        description="Last 200 lines of stderr",
    )
    pushed: bool | None = Field(default=None, description="Whether git push succeeded")
    plan_hash: str = Field(default="00000000", description="MD5 of plan.md")
    unchecked_tasks: int = Field(default=0, ge=0, description="Unchecked task count")
    spec_modified: bool = Field(
        default=False,
        description="Whether spec.md was modified and reverted",
    )
    integrity_repairs: IntegrityRepairs | None = Field(default=None)
    new_tasks: list[Finding] = Field(
        default_factory=list,
        description="Tasks from audit findings",
    )
    findings: list[Finding] = Field(
        default_factory=list,
        description="Structured findings",
    )
    token_usage: TokenUsage | None = Field(
        default=None,
        description="Token usage data",
    )


class TeardownResult(BaseModel):
    """Result data from a successful teardown step."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    vm_released: bool = Field(default=True, description="Whether the VM was released")
    duration_secs: int = Field(default=0, ge=0, description="Teardown duration in seconds")
    estimated_cost: float | None = Field(
        default=None,
        ge=0.0,
        description="Estimated total VM cost in USD",
    )


class DryRunStepPayload(BaseModel):
    """Payload for a dry-run step."""

    model_config = ConfigDict(extra="forbid")

    dispatch: Dispatch = Field(..., description="Full dispatch payload")


class DryRunResult(BaseModel):
    """Result of a dry-run step."""

    model_config = ConfigDict(extra="forbid")

    provider: str = Field(..., description="VM provider that would be used")
    server_type: str | None = Field(default=None, description="Server type")
    estimated_cost_hourly: float | None = Field(default=None, description="Estimated hourly cost")
    would_provision: bool = Field(..., description="Whether provisioning would proceed")
