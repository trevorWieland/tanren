"""Data types for the ExecutionEnvironment protocol."""

from __future__ import annotations

from pathlib import Path
from typing import Literal

from pydantic import BaseModel, ConfigDict, Field

from worker_manager.adapters.remote_types import VMHandle, WorkspacePath
from worker_manager.env.environment_schema import EnvironmentProfile
from worker_manager.env.validator import EnvReport
from worker_manager.postflight import PostflightResult
from worker_manager.preflight import PreflightResult
from worker_manager.schemas import Outcome, Result


class LocalEnvironmentRuntime(BaseModel):
    """Runtime state for local execution environments."""

    model_config = ConfigDict(extra="forbid", arbitrary_types_allowed=True)

    kind: Literal["local"] = Field(default="local")
    preflight_result: PreflightResult | None = Field(default=None)
    task_env: dict[str, str] = Field(default_factory=dict)
    env_report: EnvReport | None = Field(default=None)


class RemoteEnvironmentRuntime(BaseModel):
    """Runtime state for remote SSH execution environments."""

    model_config = ConfigDict(extra="forbid", arbitrary_types_allowed=True)

    kind: Literal["remote"] = Field(default="remote")
    vm_handle: VMHandle = Field(...)
    connection: object = Field(...)
    workspace_path: WorkspacePath = Field(...)
    profile: EnvironmentProfile = Field(...)
    teardown_commands: tuple[str, ...] = Field(default_factory=tuple)
    provision_start: float = Field(..., ge=0.0)
    workflow_id: str = Field(...)


class CustomEnvironmentRuntime(BaseModel):
    """Generic runtime state for custom execution environments."""

    model_config = ConfigDict(extra="forbid")

    kind: Literal["custom"] = Field(default="custom")
    adapter: str = Field(..., min_length=1)
    metadata: dict[str, str] = Field(default_factory=dict)


class EnvironmentHandle(BaseModel):
    """Handle returned by provision() — carries typed runtime context."""

    model_config = ConfigDict(extra="forbid", arbitrary_types_allowed=True)

    env_id: str = Field(...)
    worktree_path: Path = Field(...)
    branch: str = Field(...)
    project: str = Field(...)
    runtime: LocalEnvironmentRuntime | RemoteEnvironmentRuntime | CustomEnvironmentRuntime = Field(
        ..., discriminator="kind"
    )


class PhaseResult(BaseModel):
    """Result of execute() — carries all data needed to construct a Result."""

    model_config = ConfigDict(extra="forbid")

    outcome: Outcome = Field(...)
    signal: str | None = Field(default=None)
    exit_code: int = Field(...)
    stdout: str | None = Field(default=None)
    duration_secs: int = Field(..., ge=0)
    preflight_passed: bool = Field(...)
    postflight_result: PostflightResult | None = Field(default=None)
    env_report: EnvReport | None = Field(default=None)
    gate_output: str | None = Field(default=None)
    unchecked_tasks: int = Field(default=0, ge=0)
    plan_hash: str = Field(default="00000000")
    retries: int = Field(default=0, ge=0)


class AccessInfo(BaseModel):
    """Connection info for debugging a running environment."""

    model_config = ConfigDict(extra="forbid")

    ssh: str | None = Field(default=None)
    vscode: str | None = Field(default=None)
    working_dir: str | None = Field(default=None)
    status: str = Field(default="running")


class ProvisionError(Exception):
    """Raised when provision() fails (env validation or preflight)."""

    def __init__(
        self,
        result: Result,
        preflight_result: PreflightResult | None = None,
    ) -> None:
        self.result = result
        self.preflight_result = preflight_result
        super().__init__(str(result.tail_output))
