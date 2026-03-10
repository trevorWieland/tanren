"""Worker manager configuration from environment variables with WM_ prefix."""

import os
from pathlib import Path

from pydantic import BaseModel, Field


def _expand(path: str) -> str:
    return str(Path(path).expanduser())


class Config(BaseModel):
    """Worker manager configuration loaded from environment variables."""

    ipc_dir: str = Field(
        description="IPC directory path for coordinator group",
    )
    github_dir: str = Field(
        description="Root directory containing git repositories",
    )
    commands_dir: str = Field(
        default=".claude/commands/tanren",
        description="Relative path to tanren commands within a project",
    )
    poll_interval: float = Field(
        default=5.0,
        description="Seconds between dispatch directory polls",
    )
    heartbeat_interval: float = Field(
        default=30.0,
        description="Seconds between heartbeat file updates",
    )
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
    roles_config_path: str | None = Field(
        default=None,
        description="Path to roles YAML config",
    )
    data_dir: str = Field(
        description="Directory for worker manager runtime state",
    )
    worktree_registry_path: str = Field(
        description="Path to worktrees.json registry file",
    )
    max_opencode: int = Field(
        default=1,
        description="Maximum concurrent opencode processes",
    )
    max_codex: int = Field(
        default=1,
        description="Maximum concurrent codex processes",
    )
    max_gate: int = Field(
        default=3,
        description="Maximum concurrent gate (bash) processes",
    )
    events_db: str | None = Field(
        default=None,
        description="Path to SQLite events DB (enables event emission)",
    )

    @classmethod
    def from_env(cls) -> Config:
        """Load configuration from WM_ prefixed environment variables."""
        ipc_dir = _expand(os.environ.get("WM_IPC_DIR", "~/github/nanoclaw/data/ipc/discord_main"))
        github_dir = _expand(os.environ.get("WM_GITHUB_DIR", "~/github"))
        data_dir = _expand(os.environ.get("WM_DATA_DIR", "~/.local/share/tanren-worker"))

        return cls(
            ipc_dir=ipc_dir,
            github_dir=github_dir,
            commands_dir=os.environ.get("WM_COMMANDS_DIR", ".claude/commands/tanren"),
            poll_interval=float(os.environ.get("WM_POLL_INTERVAL", "5.0")),
            heartbeat_interval=float(os.environ.get("WM_HEARTBEAT_INTERVAL", "30.0")),
            opencode_path=os.environ.get("WM_OPENCODE_PATH", "opencode"),
            codex_path=os.environ.get("WM_CODEX_PATH", "codex"),
            claude_path=os.environ.get("WM_CLAUDE_PATH", "claude"),
            roles_config_path=os.environ.get("WM_ROLES_CONFIG_PATH"),
            data_dir=data_dir,
            worktree_registry_path=os.environ.get(
                "WM_WORKTREE_REGISTRY_PATH",
                str(Path(data_dir) / "worktrees.json"),
            ),
            max_opencode=int(os.environ.get("WM_MAX_OPENCODE", "1")),
            max_codex=int(os.environ.get("WM_MAX_CODEX", "1")),
            max_gate=int(os.environ.get("WM_MAX_GATE", "3")),
            events_db=os.environ.get("WM_EVENTS_DB"),
        )
