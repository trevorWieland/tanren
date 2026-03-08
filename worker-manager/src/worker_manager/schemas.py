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
    GATE = "gate"
    SETUP = "setup"
    CLEANUP = "cleanup"


class Cli(StrEnum):
    """CLI tool types from PROTOCOL.md Section 2."""

    OPENCODE = "opencode"
    CODEX = "codex"
    BASH = "bash"


class Outcome(StrEnum):
    """Result outcomes from PROTOCOL.md Section 3."""

    SUCCESS = "success"
    FAIL = "fail"
    BLOCKED = "blocked"
    ERROR = "error"
    TIMEOUT = "timeout"


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
    gate_output: str | None = Field(description="Last 100 lines of gate output (gate phases only)")
    tail_output: str | None = Field(
        description="Last 50 lines of output (non-success outcomes only)"
    )
    unchecked_tasks: int = Field(description="Count of unchecked Task N lines in plan.md")
    plan_hash: str = Field(description="MD5 of plan.md (first 8 hex chars)")
    spec_modified: bool = Field(description="True if spec.md was modified and reverted")


class Nudge(BaseModel):
    """Nudge message written to input/ to notify coordinator."""

    model_config = ConfigDict(extra="forbid")

    type: str = Field(default="workflow_result", description="Nudge type identifier")
    workflow_id: str = Field(description="Workflow that produced the result")


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
