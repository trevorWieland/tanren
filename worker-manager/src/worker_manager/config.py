"""Worker manager configuration from environment variables with WM_ prefix."""

import logging
import os
from collections.abc import Sequence
from pathlib import Path
from typing import Protocol, runtime_checkable

from dotenv import dotenv_values
from pydantic import BaseModel, ConfigDict, Field

logger = logging.getLogger(__name__)


def _expand(path: str) -> str:
    return str(Path(path).expanduser())


def _expand_optional(path: str | None) -> str | None:
    if not path or not path.strip():
        return None
    return _expand(path)


@runtime_checkable
class ConfigSource(Protocol):
    """Provide WM_* configuration values from an external source.

    Implementations load configuration from different backends
    (dotenv files, Vault, SSM). Resolution in Config.from_env():
    sources provide base values, os.environ overrides.
    All required fields must be present — no built-in defaults.

    Default implementation: DotenvConfigSource.
    """

    def load(self) -> dict[str, str]: ...


class DotenvConfigSource:
    """Load config from a dotenv file.

    Default: $XDG_CONFIG_HOME/tanren/tanren.env (~/.config/tanren/tanren.env).
    """

    def __init__(self, path: Path | None = None) -> None:
        if path is None:
            xdg = os.environ.get("XDG_CONFIG_HOME", str(Path.home() / ".config"))
            path = Path(xdg).expanduser() / "tanren" / "tanren.env"
        self._path = path

    def load(self) -> dict[str, str]:
        if not self._path.exists():
            logger.debug("No config file at %s — skipping", self._path)
            return {}
        values = dotenv_values(self._path)
        loaded = {k: v for k, v in values.items() if v is not None}
        logger.debug("Loaded %d config values from %s", len(loaded), self._path)
        return loaded


_REQUIRED_KEYS = (
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
)

_OPTIONAL_KEYS = ("WM_ROLES_CONFIG_PATH", "WM_EVENTS_DB", "WM_REMOTE_CONFIG")

_WM_KEYS = frozenset((*_REQUIRED_KEYS, *_OPTIONAL_KEYS))


def load_config_env(source: ConfigSource | None = None) -> None:
    """Load WM_* config into os.environ from the given source (default: tanren.env).

    Only sets WM_* variables not already present in os.environ (env wins).
    Non-WM keys from the source file are silently ignored to prevent
    leaking credentials or path overrides into child processes.
    """
    src = source or DotenvConfigSource()
    values = src.load()
    for key, value in values.items():
        if key in _WM_KEYS and key not in os.environ:
            os.environ[key] = value


class Config(BaseModel):
    """Worker manager configuration loaded from environment variables."""

    model_config = ConfigDict(extra="forbid")

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
    remote_config_path: str | None = Field(
        default=None,
        description="Path to remote.yml (enables remote execution)",
    )

    @classmethod
    def from_env(cls, sources: Sequence[ConfigSource] = ()) -> Config:
        """Load configuration from sources and environment variables.

        Sources provide base values. Environment variables override.
        All required WM_* fields must be present — no built-in defaults.
        """
        # 1. Collect values from sources
        resolved: dict[str, str] = {}
        for source in sources:
            resolved.update(source.load())

        # 2. Environment variables override source values
        for key in (*_REQUIRED_KEYS, *_OPTIONAL_KEYS):
            env_val = os.environ.get(key)
            if env_val is not None:
                resolved[key] = env_val

        # 3. Validate — all required keys must be present
        missing = [k for k in _REQUIRED_KEYS if not resolved.get(k, "").strip()]
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
            roles_config_path=_expand_optional(resolved.get("WM_ROLES_CONFIG_PATH")),
            worktree_registry_path=_expand(resolved["WM_WORKTREE_REGISTRY_PATH"]),
            max_opencode=int(resolved["WM_MAX_OPENCODE"]),
            max_codex=int(resolved["WM_MAX_CODEX"]),
            max_gate=int(resolved["WM_MAX_GATE"]),
            events_db=_expand_optional(resolved.get("WM_EVENTS_DB")),
            remote_config_path=_expand_optional(resolved.get("WM_REMOTE_CONFIG")),
        )
