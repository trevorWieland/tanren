"""GCP Secret Manager SecretProvider."""

from __future__ import annotations

import asyncio
import logging
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import types

logger = logging.getLogger(__name__)


def _import_secret_manager() -> types.ModuleType:
    """Import and return the google.cloud.secretmanager module at runtime.

    Returns:
        The google.cloud.secretmanager module.

    Raises:
        ImportError: If the google-cloud-secret-manager package is not installed.
    """
    try:
        import google.cloud.secretmanager as _sm  # noqa: PLC0415

        return _sm
    except ImportError:
        raise ImportError(
            "google-cloud-secret-manager is required for GCP secrets. "
            "Install it with: uv sync --extra gcp"
        ) from None


class GCPSecretManagerProvider:
    """Fetch secrets from GCP Secret Manager using Application Default Credentials.

    Caches fetched secrets for the lifetime of the provider instance.
    """

    def __init__(self, project_id: str) -> None:
        """Initialize with a GCP project ID."""
        self._project_id = project_id
        self._sm = _import_secret_manager()
        self._client = self._sm.SecretManagerServiceClient()
        self._cache: dict[str, str] = {}

    async def get_secret(self, secret_id: str, *, version: str = "latest") -> str | None:
        """Fetch a secret from GCP Secret Manager.

        Returns:
            The secret value as a string, or None if the secret is not found
            or an error occurs.
        """
        cache_key = f"{secret_id}:{version}"
        if cache_key in self._cache:
            return self._cache[cache_key]

        name = f"projects/{self._project_id}/secrets/{secret_id}/versions/{version}"
        try:
            response = await asyncio.to_thread(
                self._client.access_secret_version,
                request={"name": name},
            )
            value = response.payload.data.decode("UTF-8")
            self._cache[cache_key] = value
            return value
        except Exception:
            logger.warning(
                "Failed to fetch secret %s from GCP Secret Manager",
                secret_id,
                exc_info=True,
            )
            return None

    async def list_secrets(self) -> list[str]:
        """List secret IDs in the GCP project.

        Returns:
            List of secret ID strings, or an empty list on error.
        """
        parent = f"projects/{self._project_id}"
        try:
            secrets = await asyncio.to_thread(
                lambda: list(self._client.list_secrets(request={"parent": parent}))
            )
            return [s.name.rsplit("/", 1)[-1] for s in secrets]
        except Exception:
            logger.warning(
                "Failed to list secrets from GCP Secret Manager",
                exc_info=True,
            )
            return []
