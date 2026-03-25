"""Worker configuration from environment variables with WM_ prefix."""

import logging
import os
from pathlib import Path
from typing import Protocol, runtime_checkable

from dotenv import dotenv_values

from tanren_core.worker_config import _WC_OPTIONAL_KEYS, _WC_REQUIRED_KEYS

logger = logging.getLogger(__name__)


@runtime_checkable
class ConfigSource(Protocol):
    """Provide WM_* configuration values from an external source.

    Implementations load configuration from different backends
    (dotenv files, Vault, SSM). Sources provide base values,
    os.environ overrides.  All required fields must be present —
    no built-in defaults.

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


_WM_KEYS = frozenset((*_WC_REQUIRED_KEYS, *_WC_OPTIONAL_KEYS))


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
