"""Pydantic models matching PROTOCOL.md Sections 2-4."""

import re
from enum import StrEnum

from pydantic import BaseModel, ConfigDict, Field


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


class GateResult(BaseModel):
    """Record of a single gate execution for a task."""

    model_config = ConfigDict(extra="forbid")

    attempt: int
    passed: bool
    must_pass_failures: list[str] = Field(default_factory=list)
    unexpected_passes: list[str] = Field(default_factory=list)


class AuditResult(BaseModel):
    """Record of a single audit execution for a task."""

    model_config = ConfigDict(extra="forbid")

    attempt: int
    signal: str | None
    findings: list = Field(default_factory=list)


class TaskState(BaseModel):
    """State of a single task in progress.json."""

    model_config = ConfigDict(extra="forbid")

    id: int
    title: str
    status: TaskStatus = TaskStatus.PENDING
    attempts: int = 0
    gate_results: list[GateResult] = Field(default_factory=list)
    audit_results: list[AuditResult] = Field(default_factory=list)
    gate_expectations: dict | None = None
    source: str | None = None


class ProgressState(BaseModel):
    """Full progress.json state for a spec's orchestration."""

    model_config = ConfigDict(extra="forbid")

    spec_id: str
    version: int = 1
    created_at: str
    updated_at: str
    tasks: list[TaskState]


class FindingSeverity(StrEnum):
    """Severity levels for structured findings."""

    FIX = "fix"
    NOTE = "note"
    QUESTION = "question"


class Finding(BaseModel):
    """Single finding from an audit/demo phase."""

    model_config = ConfigDict(extra="forbid")

    title: str
    description: str = ""
    severity: FindingSeverity = FindingSeverity.FIX
    affected_files: list[str] = Field(default_factory=list)
    line_numbers: list[int] = Field(default_factory=list)


class FindingsOutput(BaseModel):
    """Structured findings output from audit-task or run-demo."""

    model_config = ConfigDict(extra="forbid")

    signal: str
    findings: list[Finding] = Field(default_factory=list)


class InvestigationRootCause(BaseModel):
    """Single root cause identified by the INVESTIGATE phase."""

    model_config = ConfigDict(extra="forbid")

    description: str
    confidence: str
    affected_files: list[str] = Field(default_factory=list)
    category: str
    suggested_tasks: list[dict] = Field(default_factory=list)


class InvestigationReport(BaseModel):
    """Output of the INVESTIGATE phase."""

    model_config = ConfigDict(extra="forbid")

    trigger: str
    root_causes: list[InvestigationRootCause] = Field(default_factory=list)
    unrelated_failures: list[dict] = Field(default_factory=list)
    escalation_needed: bool = False
    escalation_reason: str | None = None


class GateExpectation(BaseModel):
    """Per-task gate postconditions."""

    model_config = ConfigDict(extra="forbid")

    must_pass: list[str] = Field(default_factory=list)
    expect_fail: list[str] = Field(default_factory=list)
    skip: list[str] = Field(default_factory=list)
    gate_command_override: str | None = None


class TaskGateExpectation(BaseModel):
    """Gate expectations for a specific task, as written by shape-spec."""

    model_config = ConfigDict(extra="forbid")

    task_id: int
    title: str
    gate: GateExpectation


class Dispatch(BaseModel):
    """Dispatch schema from PROTOCOL.md Section 2."""

    model_config = ConfigDict(extra="forbid")

    workflow_id: str = Field(description="Unique workflow identifier: wf-{project}-{issue}-{epoch}")
    phase: Phase = Field(description="Dispatch phase type")
    project: str = Field(description="Project name (matches repo name)")
    spec_folder: str = Field(description="Relative path from project root to spec folder")
    branch: str = Field(description="Git branch name for this workflow")
    cli: Cli = Field(description="CLI tool to use")
    model: str | None = Field(description="Model identifier passed to CLI, null for gates")
    gate_cmd: str | None = Field(description="Shell command for gate phases, null for agent phases")
    context: str | None = Field(description="Extra context passed to the agent")
    timeout: int = Field(description="Maximum execution time in seconds")


class Result(BaseModel):
    """Result schema from PROTOCOL.md Section 3."""

    model_config = ConfigDict(extra="forbid")

    workflow_id: str = Field(description="Matches the dispatch's workflow_id")
    phase: Phase = Field(description="Matches the dispatch's phase")
    outcome: Outcome = Field(description="Result outcome")
    signal: str | None = Field(description="Raw agent signal or null")
    exit_code: int = Field(description="Process exit code")
    duration_secs: int = Field(description="Wall-clock execution time")
    gate_output: str | None = Field(
        description="Last 100/300 lines of gate stdout (success/fail); gate phases only"
    )
    tail_output: str | None = Field(
        description="Last 200 lines of stdout (agent phases always, others on non-success)"
    )
    unchecked_tasks: int = Field(description="Count of unchecked Task N lines in plan.md")
    plan_hash: str = Field(description="MD5 of plan.md (first 8 hex chars)")
    spec_modified: bool = Field(description="True if spec.md was modified and reverted")
    pushed: bool | None = Field(
        default=None,
        description="True if git push succeeded after agent phase, null for gates/setup/cleanup",
    )
    integrity_repairs: dict | None = Field(
        default=None, description="Post-flight integrity repair actions"
    )
    new_tasks: list[dict] = Field(
        default_factory=list, description="Tasks to add from audit findings"
    )
    findings: list[dict] = Field(
        default_factory=list,
        description="Structured findings from audit/demo/investigate phases",
    )


class Nudge(BaseModel):
    """Nudge message written to input/ to notify coordinator."""

    model_config = ConfigDict(extra="forbid")

    type: str = Field(default="workflow_result", description="Nudge type identifier")
    workflow_id: str = Field(description="Workflow that produced the result")


class WorkerHealth(BaseModel):
    """Worker manager health status, written each poll cycle."""

    alive: bool = Field(default=True)
    pid: int
    started_at: str
    last_poll: str
    active_processes: int
    queued_dispatches: int


class WorktreeEntry(BaseModel):
    """Single worktree entry in the registry."""

    project: str = Field(description="Project name")
    issue: int = Field(description="GitHub issue number")
    branch: str = Field(description="Git branch name")
    path: str = Field(description="Absolute path to the worktree")
    created_at: str = Field(description="ISO 8601 creation timestamp")


class WorktreeRegistry(BaseModel):
    """Worktree registry for isolation enforcement."""

    worktrees: dict[str, WorktreeEntry] = Field(
        default_factory=dict, description="Map of workflow_id to worktree entry"
    )


def parse_issue_from_workflow_id(workflow_id: str) -> int:
    """Extract issue number from workflow_id format: wf-{project}-{issue}-{epoch}.

    The project name may contain hyphens, so we match from the end:
    the last segment is epoch (digits), second-to-last is issue (digits).
    """
    match = re.match(r"^wf-(.+)-(\d+)-(\d+)$", workflow_id)
    if not match:
        raise ValueError(f"Invalid workflow_id format: {workflow_id}")
    return int(match.group(2))
