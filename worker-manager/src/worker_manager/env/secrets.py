"""Manage ~/.aegis/secrets.env: read/write secrets with redaction."""

import os
from pathlib import Path

from dotenv import dotenv_values


def redact(value: str) -> str:
    """Redact a value for safe display: first 4 chars + '...'."""
    if len(value) < 6:
        return "****"
    return value[:4] + "..."


def ensure_aegis_dir(aegis_dir: Path | None = None) -> Path:
    """Create ~/.aegis with chmod 700 (lazy — only on first write)."""
    d = aegis_dir or Path.home() / ".aegis"
    d.mkdir(mode=0o700, parents=True, exist_ok=True)
    return d


def set_secret(
    key: str,
    value: str,
    aegis_dir: Path | None = None,
) -> Path:
    """Write/update a key in ~/.aegis/secrets.env. Creates file with chmod 600."""
    d = ensure_aegis_dir(aegis_dir)
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
    aegis_dir: Path | None = None,
) -> list[tuple[str, str]]:
    """Return (key, redacted_value) pairs from ~/.aegis/secrets.env."""
    d = aegis_dir or Path.home() / ".aegis"
    secrets_path = d / "secrets.env"

    if not secrets_path.exists():
        return []

    values = dotenv_values(secrets_path)
    return [(k, redact(v)) for k, v in values.items() if v is not None]
