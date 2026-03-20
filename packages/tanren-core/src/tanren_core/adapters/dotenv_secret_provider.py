"""Dotenv-backed SecretProvider — wraps existing secrets.env + secrets.d/*.env."""

from __future__ import annotations

import asyncio
from typing import TYPE_CHECKING

from dotenv import dotenv_values

from tanren_core.env.secrets import DEFAULT_SECRETS_DIR

if TYPE_CHECKING:
    from pathlib import Path


class DotenvSecretProvider:
    """Read secrets from dotenv files on disk.

    Loads secrets.env and secrets.d/*.env on first access, caches in memory.
    This is the backward-compatible default SecretProvider.
    """

    def __init__(self, secrets_dir: Path | None = None) -> None:
        """Initialize with an optional secrets directory path."""
        self._secrets_dir = secrets_dir or DEFAULT_SECRETS_DIR
        self._cache: dict[str, str] | None = None

    def _load(self) -> dict[str, str]:
        """Load secrets from disk (cached after first call).

        Returns:
            Dict of secret key-value pairs.
        """
        if self._cache is not None:
            return self._cache

        merged: dict[str, str] = {}

        secrets_path = self._secrets_dir / "secrets.env"
        if secrets_path.exists():
            values = dotenv_values(secrets_path)
            merged.update({k: v for k, v in values.items() if v is not None})

        secrets_d = self._secrets_dir / "secrets.d"
        if secrets_d.is_dir():
            for env_file in sorted(secrets_d.glob("*.env")):
                values = dotenv_values(env_file)
                merged.update({k: v for k, v in values.items() if v is not None})

        self._cache = merged
        return merged

    async def get_secret(self, secret_id: str, *, version: str = "latest") -> str | None:  # noqa: ARG002 — required by protocol interface
        """Look up a secret by key name in the dotenv files.

        Returns:
            The secret value, or None if the key is not found.
        """
        secrets = await asyncio.to_thread(self._load)
        return secrets.get(secret_id)

    async def list_secrets(self) -> list[str]:
        """List all secret keys available in the dotenv files.

        Returns:
            List of secret key strings.
        """
        secrets = await asyncio.to_thread(self._load)
        return list(secrets.keys())
