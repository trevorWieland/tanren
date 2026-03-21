"""Worker manager configuration from environment variables with WM_ prefix.

.. deprecated::
    ``Config`` is a transitional alias for ``WorkerConfig``.
    Use ``WorkerConfig`` directly in new code.  ``Config`` will be
    removed in Phase 8 of the stateless API refactor.
"""

import logging
import os
from pathlib import Path
from typing import TYPE_CHECKING, Protocol, runtime_checkable

from dotenv import dotenv_values
from pydantic import ConfigDict

from tanren_core.worker_config import WorkerConfig, _expand, _expand_optional, _pg_or_expand

if TYPE_CHECKING:
    from collections.abc import Sequence

logger = logging.getLogger(__name__)


@runtime_checkable
class ConfigSource(Protocol):
    """Provide WM_* configuration values from an external source.

    Implementations load configuration from different backends
    (dotenv files, Vault, SSM). Resolution in Config.from_env():
    sources provide base values, os.environ overrides.
    All required fields must be present — no built-in defaults.

    Default implementation: DotenvConfigSource.
    """

    def load(self) -> dict[str, str]:
        """Load and return WM_* configuration key-value pairs."""
        ...


class DotenvConfigSource:
    """Load config from a dotenv file.

    Default: $XDG_CONFIG_HOME/tanren/tanren.env (~/.config/tanren/tanren.env).
    """

    def __init__(self, path: Path | None = None) -> None:
        """Initialize with an optional path to a dotenv config file."""
        if path is None:
            xdg = os.environ.get("XDG_CONFIG_HOME", str(Path.home() / ".config"))
            path = Path(xdg).expanduser() / "tanren" / "tanren.env"
        self._path = path

    def load(self) -> dict[str, str]:
        """Load WM_* config values from the dotenv file.

        Returns:
            Dict of loaded config key-value pairs.
        """
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
    "WM_ROLES_CONFIG_PATH",
)

_OPTIONAL_KEYS = (
    "WM_EVENTS_DB",
    "WM_REMOTE_CONFIG",
    "WM_CCUSAGE_CLAUDE_CMD",
    "WM_CCUSAGE_CODEX_CMD",
    "WM_CCUSAGE_OPENCODE_CMD",
)

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


class Config(WorkerConfig):
    """Worker manager configuration loaded from environment variables.

    .. deprecated::
        Transitional alias for ``WorkerConfig``.  Will be removed in
        Phase 8 cleanup.  Use ``WorkerConfig`` directly in new code.
    """

    model_config = ConfigDict(extra="forbid")

    @classmethod
    def from_env(cls, sources: Sequence[ConfigSource] = ()) -> Config:
        """Load configuration from sources and environment variables.

        Sources provide base values. Environment variables override.
        All required WM_* fields must be present -- no built-in defaults.

        Returns:
            Validated Config instance.

        Raises:
            ValueError: If required configuration keys are missing.
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

        max_opencode = int(resolved["WM_MAX_OPENCODE"])
        max_codex = int(resolved["WM_MAX_CODEX"])

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
            max_opencode=max_opencode,
            max_codex=max_codex,
            max_impl=max_opencode,
            max_audit=max_codex,
            max_gate=int(resolved["WM_MAX_GATE"]),
            events_db=_pg_or_expand(resolved.get("WM_EVENTS_DB")),
            db_url=_pg_or_expand(resolved.get("WM_EVENTS_DB")),
            remote_config_path=_expand_optional(resolved.get("WM_REMOTE_CONFIG")),
            ccusage_claude_cmd=resolved.get("WM_CCUSAGE_CLAUDE_CMD", "npx ccusage"),
            ccusage_codex_cmd=resolved.get("WM_CCUSAGE_CODEX_CMD", "npx @ccusage/codex"),
            ccusage_opencode_cmd=resolved.get("WM_CCUSAGE_OPENCODE_CMD", "npx @ccusage/opencode"),
        )
