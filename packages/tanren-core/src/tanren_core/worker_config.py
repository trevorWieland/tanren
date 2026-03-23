"""Worker configuration with lane-based concurrency and filesystem paths.

``WorkerConfig`` carries all fields a worker process needs (filesystem
paths, CLI binary paths, concurrency limits, polling intervals).
"""

from __future__ import annotations

import logging
import os
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


_WC_REQUIRED_KEYS = (
    "WM_IPC_DIR",
    "WM_GITHUB_DIR",
    "WM_DATA_DIR",
    "WM_COMMANDS_DIR",
    "WM_POLL_INTERVAL",
    "WM_HEARTBEAT_INTERVAL",
    "WM_OPENCODE_PATH",
    "WM_CODEX_PATH",
    "WM_CLAUDE_PATH",
    "WM_MAX_OPENCODE",
    "WM_MAX_CODEX",
    "WM_MAX_GATE",
    "WM_WORKTREE_REGISTRY_PATH",
    "WM_ROLES_CONFIG_PATH",
)

_WC_OPTIONAL_KEYS = (
    "WM_EVENTS_DB",
    "WM_REMOTE_CONFIG",
    "WM_CCUSAGE_CLAUDE_CMD",
    "WM_CCUSAGE_CODEX_CMD",
    "WM_CCUSAGE_OPENCODE_CMD",
)


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
        description="Max concurrent impl-lane steps",
    )
    max_audit: int = Field(
        default=1,
        ge=1,
        description="Max concurrent audit-lane steps",
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

    @classmethod
    def from_env(cls) -> WorkerConfig:
        """Load configuration from ``WM_*`` environment variables.

        Reads required and optional keys from ``os.environ``, validates
        that all required keys are present, and returns a fully-populated
        ``WorkerConfig``.

        Returns:
            Validated WorkerConfig instance.

        Raises:
            ValueError: If required configuration keys are missing.
        """
        resolved: dict[str, str] = {}
        for key in (*_WC_REQUIRED_KEYS, *_WC_OPTIONAL_KEYS):
            env_val = os.environ.get(key)
            if env_val is not None:
                resolved[key] = env_val

        missing = [k for k in _WC_REQUIRED_KEYS if not resolved.get(k, "").strip()]
        if missing:
            raise ValueError(
                f"Missing required config: {', '.join(missing)}. "
                "Set them in tanren.env or as environment variables."
            )

        return cls(
            ipc_dir=_expand(resolved["WM_IPC_DIR"]),
            github_dir=_expand(resolved["WM_GITHUB_DIR"]),
            data_dir=_expand(resolved["WM_DATA_DIR"]),
            commands_dir=resolved["WM_COMMANDS_DIR"],
            poll_interval=float(resolved["WM_POLL_INTERVAL"]),
            heartbeat_interval=float(resolved["WM_HEARTBEAT_INTERVAL"]),
            opencode_path=resolved["WM_OPENCODE_PATH"],
            codex_path=resolved["WM_CODEX_PATH"],
            claude_path=resolved["WM_CLAUDE_PATH"],
            roles_config_path=_expand(resolved["WM_ROLES_CONFIG_PATH"]),
            worktree_registry_path=_expand(resolved["WM_WORKTREE_REGISTRY_PATH"]),
            max_impl=int(resolved["WM_MAX_OPENCODE"]),
            max_audit=int(resolved["WM_MAX_CODEX"]),
            max_gate=int(resolved["WM_MAX_GATE"]),
            db_url=_pg_or_expand(resolved.get("WM_EVENTS_DB")),
            remote_config_path=_expand_optional(resolved.get("WM_REMOTE_CONFIG")),
            ccusage_claude_cmd=resolved.get("WM_CCUSAGE_CLAUDE_CMD", "npx ccusage"),
            ccusage_codex_cmd=resolved.get("WM_CCUSAGE_CODEX_CMD", "npx @ccusage/codex"),
            ccusage_opencode_cmd=resolved.get("WM_CCUSAGE_OPENCODE_CMD", "npx @ccusage/opencode"),
        )
