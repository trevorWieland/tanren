"""Data types for the ExecutionEnvironment protocol."""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from worker_manager.env.validator import EnvReport
from worker_manager.postflight import PostflightResult
from worker_manager.preflight import PreflightResult
from worker_manager.schemas import Outcome, Result


@dataclass
class EnvironmentHandle:
    """Handle returned by provision() — carries context through the lifecycle."""

    env_id: str
    worktree_path: Path
    branch: str
    project: str
    metadata: dict[str, Any] = field(default_factory=dict)
    # Internal: carry preflight/env data for PhaseResult construction
    _preflight_result: PreflightResult | None = field(default=None, repr=False)
    _task_env: dict[str, str] = field(default_factory=dict, repr=False)
    _env_report: EnvReport | None = field(default=None, repr=False)


@dataclass
class PhaseResult:
    """Result of execute() — carries all data needed to construct a Result."""

    outcome: Outcome
    signal: str | None
    exit_code: int
    stdout: str | None
    duration_secs: int
    preflight_passed: bool
    postflight_result: PostflightResult | None
    env_report: EnvReport | None
    gate_output: str | None
    # Internal tracking
    unchecked_tasks: int = 0
    plan_hash: str = "00000000"
    retries: int = 0


@dataclass
class AccessInfo:
    """Connection info for debugging a running environment."""

    ssh: str | None = None
    vscode: str | None = None
    working_dir: str | None = None
    status: str = "running"


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
