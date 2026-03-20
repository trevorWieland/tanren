"""Create SecretProvider instances from tanren.yml SecretsConfig."""

from __future__ import annotations

from typing import TYPE_CHECKING

from tanren_core.env.schema import SecretsConfig, SecretsProviderType

if TYPE_CHECKING:
    from pathlib import Path

    from tanren_core.adapters.protocols import SecretProvider


def create_secret_provider(
    config: SecretsConfig | None,
    *,
    secrets_dir: Path | None = None,
) -> SecretProvider:
    """Build a SecretProvider from the tanren.yml secrets config.

    Returns:
        DotenvSecretProvider when config is None or provider is "dotenv",
        GCPSecretManagerProvider when provider is "gcp".

    Raises:
        ValueError: If the provider type is unsupported or required settings are missing.
    """
    if config is None or config.provider == SecretsProviderType.DOTENV:
        from tanren_core.adapters.dotenv_secret_provider import (  # noqa: PLC0415
            DotenvSecretProvider,
        )

        return DotenvSecretProvider(secrets_dir=secrets_dir)

    if config.provider == SecretsProviderType.GCP:
        from tanren_core.adapters.gcp_secret_manager import (  # noqa: PLC0415
            GCPSecretManagerProvider,
        )

        project_id = config.settings.get("project_id")
        if not project_id:
            raise ValueError(
                "secrets.settings.project_id is required when secrets.provider is 'gcp'"
            )
        return GCPSecretManagerProvider(project_id=project_id)

    raise ValueError(f"Unsupported secrets provider: {config.provider}")
