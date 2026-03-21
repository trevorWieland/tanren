"""Worker configuration with lane-based concurrency and filesystem paths.

``WorkerConfig`` is the successor to ``Config`` — it carries all fields a
worker process needs (filesystem paths, CLI binary paths, concurrency
limits, polling intervals).  During the transition, ``Config`` inherits
from ``WorkerConfig`` so existing code keeps working.
"""

from __future__ import annotations

import logging
from pathlib import Path

from pydantic import BaseModel, ConfigDict, Field

logger = logging.getLogger(__name__)


def _expand(path: str) -> str:
    return str(Path(path).expanduser())


def _expand_optional(path: str | None) -> str | None:
    if not path or not path.strip():
        return None
    return _expand(path)


def _pg_or_expand(value: str | None) -> str | None:
    """Return Postgres URLs as-is; expand filesystem paths."""
    if not value or not value.strip():
        return None
    if value.lower().startswith(("postgresql://", "postgres://")):
        return value
    return _expand(value)


class WorkerConfig(BaseModel):
    """Configuration for a tanren worker process.

    Carries filesystem paths, CLI binary paths, concurrency limits,
    and polling intervals.  Loaded from ``WM_*`` environment variables
    for backward compatibility.
    """

    model_config = ConfigDict(extra="forbid")

    # Storage
    db_url: str | None = Field(
        default=None,
        description="SQLite path or postgresql:// URL for the unified store",
    )

    # Filesystem paths
    ipc_dir: str = Field(
        description="IPC directory path for coordinator group",
    )
    github_dir: str = Field(
        description="Root directory containing git repositories",
    )
    data_dir: str = Field(
        description="Directory for worker runtime state",
    )
    commands_dir: str = Field(
        default=".claude/commands/tanren",
        description="Relative path to tanren commands within a project",
    )
    roles_config_path: str = Field(
        ...,
        description="Path to roles YAML config",
    )
    remote_config_path: str | None = Field(
        default=None,
        description="Path to remote.yml (enables remote execution)",
    )
    worktree_registry_path: str = Field(
        description="Path to worktrees.json registry file",
    )

    # CLI binary paths
    opencode_path: str = Field(
        default="opencode",
        description="Path to opencode CLI binary",
    )
    codex_path: str = Field(
        default="codex",
        description="Path to codex CLI binary",
    )
    claude_path: str = Field(
        default="claude",
        description="Path to Claude Code CLI binary",
    )

    # Concurrency limits (lane-based naming)
    max_impl: int = Field(
        default=1,
        ge=1,
        description="Max concurrent impl-lane steps (was max_opencode)",
    )
    max_audit: int = Field(
        default=1,
        ge=1,
        description="Max concurrent audit-lane steps (was max_codex)",
    )
    max_gate: int = Field(
        default=3,
        ge=1,
        description="Max concurrent gate-lane steps",
    )
    max_provision: int = Field(
        default=10,
        ge=1,
        description="Max concurrent provision/teardown steps",
    )

    # Legacy concurrency aliases (kept for backward compat with Config)
    max_opencode: int = Field(
        default=1,
        description="Legacy alias for max_impl",
    )
    max_codex: int = Field(
        default=1,
        description="Legacy alias for max_audit",
    )

    # Polling
    poll_interval: float = Field(
        default=5.0,
        description="Seconds between dispatch directory polls",
    )
    heartbeat_interval: float = Field(
        default=30.0,
        description="Seconds between heartbeat file updates",
    )
    poll_interval_secs: float = Field(
        default=2.0,
        ge=0.1,
        description="Seconds between queue polls (new name)",
    )

    # Worker identity
    worker_id: str = Field(
        default="",
        description="Unique worker identifier (auto-generated if empty)",
    )

    # Events DB (legacy — aliased to db_url during migration)
    events_db: str | None = Field(
        default=None,
        description="Legacy: SQLite path or postgresql:// URL for events",
    )

    # Token usage collection
    ccusage_claude_cmd: str = Field(
        default="npx ccusage",
        description="Command for ccusage (Claude)",
    )
    ccusage_codex_cmd: str = Field(
        default="npx @ccusage/codex",
        description="Command for @ccusage/codex",
    )
    ccusage_opencode_cmd: str = Field(
        default="npx @ccusage/opencode",
        description="Command for @ccusage/opencode",
    )

    @property
    def checkpoints_dir(self) -> str:
        """Directory for dispatch checkpoint files, derived from data_dir."""
        return str(Path(self.data_dir) / "checkpoints")
