"""Manage secrets.env: read/write secrets with redaction.

Default location: $XDG_CONFIG_HOME/tanren/secrets.env (falls back to ~/.config/tanren/).
"""

import os
from pathlib import Path

from dotenv import dotenv_values

_xdg_config = os.environ.get("XDG_CONFIG_HOME", str(Path.home() / ".config"))
DEFAULT_SECRETS_DIR = Path(_xdg_config) / "tanren"


def redact(value: str) -> str:
    """Redact a value for safe display: first 4 chars + '...'."""
    if len(value) < 6:
        return "****"
    return value[:4] + "..."


def ensure_secrets_dir(secrets_dir: Path | None = None) -> Path:
    """Create secrets directory with chmod 700 (lazy — only on first write)."""
    d = secrets_dir or DEFAULT_SECRETS_DIR
    d.mkdir(mode=0o700, parents=True, exist_ok=True)
    return d


def set_secret(
    key: str,
    value: str,
    secrets_dir: Path | None = None,
) -> Path:
    """Write/update a key in secrets.env. Creates file with chmod 600."""
    d = ensure_secrets_dir(secrets_dir)
    secrets_path = d / "secrets.env"

    # Read existing entries
    existing: dict[str, str | None] = {}
    if secrets_path.exists():
        existing = dict(dotenv_values(secrets_path))

    existing[key] = value

    # Write all entries
    lines = [f'{k}="{v}"' for k, v in existing.items() if v is not None]
    secrets_path.write_text("\n".join(lines) + "\n")
    os.chmod(secrets_path, 0o600)

    return secrets_path


def list_secrets(
    secrets_dir: Path | None = None,
) -> list[tuple[str, str]]:
    """Return (key, redacted_value) pairs from secrets.env."""
    d = secrets_dir or DEFAULT_SECRETS_DIR
    secrets_path = d / "secrets.env"

    if not secrets_path.exists():
        return []

    values = dotenv_values(secrets_path)
    return [(k, redact(v)) for k, v in values.items() if v is not None]
