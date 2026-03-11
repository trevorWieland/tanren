"""SecretLoader — scoped secret loading for remote execution."""

from __future__ import annotations

import os
from pathlib import Path

from dotenv import dotenv_values, load_dotenv
from pydantic import BaseModel, ConfigDict, Field

from worker_manager.adapters.remote_types import SecretBundle
from worker_manager.env.secrets import DEFAULT_SECRETS_DIR


class SecretConfig(BaseModel):
    """Configuration for secret loading."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    developer_secrets_path: str = Field(default=str(DEFAULT_SECRETS_DIR / "secrets.env"))
    infrastructure_env_vars: tuple[str, ...] = Field(default=("GIT_TOKEN",))


class SecretLoader:
    """Load and bundle secrets for remote injection.

    Separates secrets into three scopes:
    - developer: from the developer's secrets.env file
    - project: from the project's .env file
    - infrastructure: from environment variables (e.g., GIT_TOKEN)
    """

    def __init__(self, config: SecretConfig | None = None) -> None:
        self._config = config or SecretConfig()

    def autoload_into_env(self, *, override: bool = False) -> None:
        """Load developer secrets into process env."""
        path = Path(self._config.developer_secrets_path).expanduser()
        if not path.exists():
            return
        load_dotenv(dotenv_path=path, override=override)

    def load_developer(self) -> dict[str, str]:
        """Load developer secrets from secrets.env file."""
        path = Path(self._config.developer_secrets_path).expanduser()
        if not path.exists():
            return {}
        values = dotenv_values(path)
        return {k: v for k, v in values.items() if v is not None}

    def load_infrastructure(self) -> dict[str, str]:
        """Load infrastructure secrets from environment variables."""
        result: dict[str, str] = {}
        for var in self._config.infrastructure_env_vars:
            val = os.environ.get(var)
            if val is not None:
                result[var] = val
        return result

    def build_bundle(self, project_secrets: dict[str, str] | None = None) -> SecretBundle:
        """Build a SecretBundle from all secret sources."""
        return SecretBundle(
            developer=self.load_developer(),
            project=project_secrets or {},
            infrastructure=self.load_infrastructure(),
        )
