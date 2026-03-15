"""Pydantic models matching PROTOCOL.md Sections 2-4."""

import re
from enum import StrEnum

from pydantic import BaseModel, ConfigDict, Field, JsonValue

from tanren_core.postflight import IntegrityRepairs


class Phase(StrEnum):
    """Dispatch phase types from PROTOCOL.md Section 2."""

    DO_TASK = "do-task"
    AUDIT_TASK = "audit-task"
    RUN_DEMO = "run-demo"
    AUDIT_SPEC = "audit-spec"
    INVESTIGATE = "investigate"
    GATE = "gate"
    SETUP = "setup"
    CLEANUP = "cleanup"


class Cli(StrEnum):
    """CLI tool types from PROTOCOL.md Section 2."""

    OPENCODE = "opencode"
    CODEX = "codex"
    CLAUDE = "claude"
    BASH = "bash"


class AuthMode(StrEnum):
    """Authentication mode for agent CLI backends."""

    API_KEY = "api_key"
    OAUTH = "oauth"
    SUBSCRIPTION = "subscription"


class Outcome(StrEnum):
    """Result outcomes from PROTOCOL.md Section 3."""

    SUCCESS = "success"
    FAIL = "fail"
    BLOCKED = "blocked"
    ERROR = "error"
    TIMEOUT = "timeout"


class TaskStatus(StrEnum):
    """Progress tracking status for individual tasks."""

    PENDING = "pending"
    IN_PROGRESS = "in_progress"
    GATE_PASSED = "gate_passed"
    COMPLETED = "completed"
    BLOCKED = "blocked"
    FAILED = "failed"


class FindingSeverity(StrEnum):
    """Severity levels for structured findings."""

    FIX = "fix"
    NOTE = "note"
    QUESTION = "question"


class Finding(BaseModel):
    """Single finding from an audit/demo phase."""

    model_config = ConfigDict(extra="forbid")

    title: str = Field(...)
    description: str = Field(default="")
    severity: FindingSeverity = Field(default=FindingSeverity.FIX)
    affected_files: list[str] = Field(default_factory=list)
    line_numbers: list[int] = Field(default_factory=list)


class FindingsOutput(BaseModel):
    """Structured findings output from audit-task or run-demo."""

    model_config = ConfigDict(extra="forbid")

    signal: str = Field(...)
    findings: list[Finding] = Field(default_factory=list)


class GateResult(BaseModel):
    """Record of a single gate execution for a task."""

    model_config = ConfigDict(extra="forbid")

    attempt: int = Field(..., ge=0)
    passed: bool = Field(...)
    must_pass_failures: list[str] = Field(default_factory=list)
    unexpected_passes: list[str] = Field(default_factory=list)


class GateExpectation(BaseModel):
    """Per-task gate postconditions."""

    model_config = ConfigDict(extra="forbid")

    must_pass: list[str] = Field(default_factory=list)
    expect_fail: list[str] = Field(default_factory=list)
    skip: list[str] = Field(default_factory=list)
    gate_command_override: str | None = Field(default=None)


class TaskGateExpectation(BaseModel):
    """Gate expectations for a specific task, as written by shape-spec."""

    model_config = ConfigDict(extra="forbid")

    task_id: int = Field(...)
    title: str = Field(...)
    gate: GateExpectation = Field(...)


class AuditResult(BaseModel):
    """Record of a single audit execution for a task."""

    model_config = ConfigDict(extra="forbid")

    attempt: int = Field(..., ge=0)
    signal: str | None = Field(default=None)
    findings: list[Finding] = Field(default_factory=list)


class TaskState(BaseModel):
    """State of a single task in progress.json."""

    model_config = ConfigDict(extra="forbid")

    id: int = Field(...)
    title: str = Field(...)
    status: TaskStatus = Field(default=TaskStatus.PENDING)
    attempts: int = Field(default=0, ge=0)
    gate_results: list[GateResult] = Field(default_factory=list)
    audit_results: list[AuditResult] = Field(default_factory=list)
    gate_expectations: GateExpectation | None = Field(default=None)
    source: str | None = Field(default=None)


class ProgressState(BaseModel):
    """Full progress.json state for a spec's orchestration."""

    model_config = ConfigDict(extra="forbid")

    spec_id: str = Field(...)
    version: int = Field(default=1, ge=1)
    created_at: str = Field(...)
    updated_at: str = Field(...)
    tasks: list[TaskState] = Field(default_factory=list)


class InvestigationRootCause(BaseModel):
    """Single root cause identified by the INVESTIGATE phase."""

    model_config = ConfigDict(extra="forbid")

    description: str = Field(...)
    confidence: str = Field(...)
    affected_files: list[str] = Field(default_factory=list)
    category: str = Field(...)
    suggested_tasks: list[dict[str, JsonValue]] = Field(default_factory=list)


class InvestigationReport(BaseModel):
    """Output of the INVESTIGATE phase."""

    model_config = ConfigDict(extra="forbid")

    trigger: str = Field(...)
    root_causes: list[InvestigationRootCause] = Field(default_factory=list)
    unrelated_failures: list[dict[str, JsonValue]] = Field(default_factory=list)
    escalation_needed: bool = Field(default=False)
    escalation_reason: str | None = Field(default=None)


class Dispatch(BaseModel):
    """Dispatch schema from PROTOCOL.md Section 2."""

    model_config = ConfigDict(extra="forbid")

    workflow_id: str = Field(
        ...,
        description="Unique workflow identifier: wf-{project}-{issue}-{epoch}",
    )
    phase: Phase = Field(..., description="Dispatch phase type")
    project: str = Field(..., description="Project name (matches repo name)")
    spec_folder: str = Field(..., description="Relative path from project root to spec folder")
    branch: str = Field(..., description="Git branch name for this workflow")
    cli: Cli = Field(..., description="CLI tool to use")
    auth: AuthMode = Field(default=AuthMode.API_KEY, description="Authentication mode for CLI")
    model: str | None = Field(default=None, description="Model identifier passed to CLI")
    gate_cmd: str | None = Field(default=None, description="Shell command for gate phases")
    context: str | None = Field(default=None, description="Extra context passed to the agent")
    timeout: int = Field(..., ge=1, description="Maximum execution time in seconds")
    environment_profile: str = Field(
        default="default",
        description="Environment profile from tanren.yml",
    )


class Result(BaseModel):
    """Result schema from PROTOCOL.md Section 3."""

    model_config = ConfigDict(extra="forbid")

    workflow_id: str = Field(..., description="Matches the dispatch's workflow_id")
    phase: Phase = Field(..., description="Matches the dispatch's phase")
    outcome: Outcome = Field(..., description="Result outcome")
    signal: str | None = Field(default=None, description="Raw agent signal or null")
    exit_code: int = Field(..., description="Process exit code")
    duration_secs: int = Field(..., ge=0, description="Wall-clock execution time")
    gate_output: str | None = Field(
        default=None,
        description="Last 100/300 lines of gate stdout (success/fail); gate phases only",
    )
    tail_output: str | None = Field(
        default=None,
        description="Last 200 lines of stdout (agent phases always, others on non-success)",
    )
    stderr_tail: str | None = Field(
        default=None,
        description="Last 200 lines of stderr (when available from remote execution)",
    )
    unchecked_tasks: int = Field(default=0, ge=0, description="Count of unchecked Task N lines")
    plan_hash: str = Field(default="00000000", description="MD5 of plan.md (first 8 hex chars)")
    spec_modified: bool = Field(..., description="True if spec.md was modified and reverted")
    pushed: bool | None = Field(
        default=None,
        description="True if git push succeeded after agent phase, null for gates/setup/cleanup",
    )
    integrity_repairs: IntegrityRepairs | None = Field(
        default=None,
        description="Post-flight integrity repair actions",
    )
    new_tasks: list[dict[str, JsonValue]] = Field(
        default_factory=list,
        description="Tasks to add from audit findings",
    )
    findings: list[dict[str, JsonValue]] = Field(
        default_factory=list,
        description="Structured findings from audit/demo/investigate phases",
    )
    token_usage: dict[str, JsonValue] | None = Field(
        default=None, description="Token usage data from ccusage"
    )


class Nudge(BaseModel):
    """Nudge message written to input/ to notify coordinator."""

    model_config = ConfigDict(extra="forbid")

    type: str = Field(default="workflow_result", description="Nudge type identifier")
    workflow_id: str = Field(..., description="Workflow that produced the result")


class WorkerHealth(BaseModel):
    """Worker manager health status, written each poll cycle."""

    model_config = ConfigDict(extra="forbid")

    alive: bool = Field(default=True)
    pid: int = Field(...)
    started_at: str = Field(...)
    last_poll: str = Field(...)
    active_processes: int = Field(...)
    queued_dispatches: int = Field(...)


class WorktreeEntry(BaseModel):
    """Single worktree entry in the registry."""

    model_config = ConfigDict(extra="forbid")

    project: str = Field(..., description="Project name")
    issue: int = Field(..., description="GitHub issue number")
    branch: str = Field(..., description="Git branch name")
    path: str = Field(..., description="Absolute path to the worktree")
    created_at: str = Field(..., description="ISO 8601 creation timestamp")


class WorktreeRegistry(BaseModel):
    """Worktree registry for isolation enforcement."""

    model_config = ConfigDict(extra="forbid")

    worktrees: dict[str, WorktreeEntry] = Field(
        default_factory=dict,
        description="Map of workflow_id to worktree entry",
    )


def parse_issue_from_workflow_id(workflow_id: str) -> int:
    """Extract issue number from workflow_id format: wf-{project}-{issue}-{epoch}.

    The project name may contain hyphens, so we match from the end:
    the last segment is epoch (digits), second-to-last is issue (digits).

    Returns:
        The issue number extracted from the workflow_id.

    Raises:
        ValueError: If the workflow_id does not match the expected format.
    """
    match = re.match(r"^wf-(.+)-(\d+)-(\d+)$", workflow_id)
    if not match:
        raise ValueError(f"Invalid workflow_id format: {workflow_id}")
    return int(match.group(2))
