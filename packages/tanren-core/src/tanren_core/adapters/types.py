"""Data types for the ExecutionEnvironment protocol."""

from __future__ import annotations

from pathlib import Path
from typing import Literal

from pydantic import BaseModel, ConfigDict, Field, SkipValidation

from tanren_core.adapters.protocols import RemoteConnection
from tanren_core.adapters.remote_types import VMHandle, WorkspacePath
from tanren_core.ccusage import TokenUsage
from tanren_core.env.environment_schema import EnvironmentProfile
from tanren_core.env.validator import EnvReport
from tanren_core.postflight import PostflightResult
from tanren_core.preflight import PreflightResult
from tanren_core.schemas import Outcome, Result


class LocalEnvironmentRuntime(BaseModel):
    """Runtime state for local execution environments."""

    model_config = ConfigDict(extra="forbid", arbitrary_types_allowed=True)

    kind: Literal["local"] = Field(default="local", description="Runtime kind discriminator")
    workflow_id: str = Field(default="", description="Workflow ID for registry cleanup on teardown")
    preflight_result: PreflightResult | None = Field(
        default=None, description="Preflight check result if available"
    )
    task_env: dict[str, str] = Field(
        default_factory=dict, description="Environment variables passed to the task process"
    )
    env_report: EnvReport | None = Field(default=None, description="Environment validation report")


class RemoteEnvironmentRuntime(BaseModel):
    """Runtime state for remote SSH execution environments."""

    model_config = ConfigDict(extra="forbid", arbitrary_types_allowed=True)

    kind: Literal["remote"] = Field(default="remote", description="Runtime kind discriminator")
    vm_handle: VMHandle = Field(..., description="Handle to the provisioned VM")
    connection: SkipValidation[RemoteConnection] = Field(
        ..., description="Active SSH connection to the VM"
    )
    workspace_path: WorkspacePath = Field(..., description="Remote workspace path details")
    profile: EnvironmentProfile = Field(
        ..., description="Environment profile used for provisioning"
    )
    teardown_commands: tuple[str, ...] = Field(
        default_factory=tuple, description="Commands to run during teardown"
    )
    provision_start: float = Field(
        ..., ge=0.0, description="Monotonic timestamp when provisioning started"
    )
    workflow_id: str = Field(..., description="Workflow that owns this environment")


class CustomEnvironmentRuntime(BaseModel):
    """Generic runtime state for custom execution environments."""

    model_config = ConfigDict(extra="forbid")

    kind: Literal["custom"] = Field(default="custom", description="Runtime kind discriminator")
    adapter: str = Field(..., min_length=1, description="Custom adapter module name")
    metadata: dict[str, str] = Field(
        default_factory=dict, description="Arbitrary adapter-specific metadata"
    )


class EnvironmentHandle(BaseModel):
    """Handle returned by provision() — carries typed runtime context."""

    model_config = ConfigDict(extra="forbid", arbitrary_types_allowed=True)

    env_id: str = Field(..., description="Unique environment identifier")
    worktree_path: Path = Field(..., description="Local or remote path to the git worktree")
    branch: str = Field(..., description="Git branch checked out in this environment")
    project: str = Field(..., description="Target project name")
    runtime: LocalEnvironmentRuntime | RemoteEnvironmentRuntime | CustomEnvironmentRuntime = Field(
        ..., discriminator="kind", description="Typed runtime context (local, remote, or custom)"
    )


class PhaseResult(BaseModel):
    """Result of execute() — carries all data needed to construct a Result."""

    model_config = ConfigDict(extra="forbid")

    outcome: Outcome = Field(..., description="Overall execution outcome")
    signal: str | None = Field(default=None, description="Signal file content if present")
    exit_code: int = Field(..., description="Process exit code")
    stdout: str | None = Field(default=None, description="Captured standard output")
    stderr: str | None = Field(default=None, description="Captured standard error")
    duration_secs: int = Field(..., ge=0, description="Wall-clock duration in seconds")
    preflight_passed: bool = Field(..., description="Whether preflight checks passed")
    postflight_result: PostflightResult | None = Field(
        default=None, description="Postflight check result if available"
    )
    env_report: EnvReport | None = Field(default=None, description="Environment validation report")
    gate_output: str | None = Field(default=None, description="Gate phase output content")
    unchecked_tasks: int = Field(default=0, ge=0, description="Number of tasks not yet verified")
    plan_hash: str = Field(
        default="00000000", description="Hash of the execution plan for change detection"
    )
    retries: int = Field(default=0, ge=0, description="Number of retry attempts performed")
    token_usage: TokenUsage | None = Field(
        default=None, description="Token usage data from the CLI session"
    )


class AccessInfo(BaseModel):
    """Connection info for debugging a running environment."""

    model_config = ConfigDict(extra="forbid")

    ssh: str | None = Field(default=None, description="SSH connection string (user@host:port)")
    vscode: str | None = Field(default=None, description="VS Code Remote SSH connection URI")
    working_dir: str | None = Field(
        default=None, description="Working directory path on the remote host"
    )
    status: str = Field(default="running", description="Current environment status")


class ProvisionError(Exception):
    """Raised when provision() fails (env validation or preflight)."""

    def __init__(
        self,
        result: Result,
        preflight_result: PreflightResult | None = None,
    ) -> None:
        """Initialize with the failed result and optional preflight result."""
        self.result = result
        self.preflight_result = preflight_result
        super().__init__(str(result.tail_output))
