"""SecretLoader — scoped secret loading for remote execution."""

from __future__ import annotations

import os
from pathlib import Path

from dotenv import dotenv_values, load_dotenv
from pydantic import BaseModel, ConfigDict, Field

from tanren_core.adapters.remote_types import SecretBundle
from tanren_core.env.secrets import DEFAULT_SECRETS_DIR
from tanren_core.schemas import Cli

_CLI_CREDENTIAL_FILES: dict[Cli, tuple[str, str]] = {
    Cli.CLAUDE: ("CLAUDE_CREDENTIALS_JSON", "claude_credentials.json"),
    Cli.CODEX: ("CODEX_AUTH_JSON", "codex_auth.json"),
}


class SecretConfig(BaseModel):
    """Configuration for secret loading."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    developer_secrets_path: str = Field(
        default=str(DEFAULT_SECRETS_DIR / "secrets.env"),
        description="Filesystem path to the developer secrets.env file",
    )
    infrastructure_env_vars: tuple[str, ...] = Field(
        default=("GIT_TOKEN",),
        description="Environment variable names to load as infrastructure secrets",
    )


class SecretLoader:
    """Load and bundle secrets for remote injection.

    Separates secrets into three scopes:
    - developer: from the developer's secrets.env file
    - project: from the project's .env file
    - infrastructure: from environment variables (e.g., GIT_TOKEN)
    """

    def __init__(
        self,
        config: SecretConfig | None = None,
        *,
        required_clis: frozenset[Cli],
    ) -> None:
        """Initialize with secret configuration and required CLIs."""
        self._config = config or SecretConfig()
        self._required_clis = required_clis

    def autoload_into_env(self, *, override: bool = False) -> None:
        """Load developer secrets into process env."""
        path = Path(self._config.developer_secrets_path).expanduser()
        if not path.exists():
            return
        load_dotenv(dotenv_path=path, override=override)

    def load_developer(self) -> dict[str, str]:
        """Load developer secrets from secrets.env file.

        Returns:
            Dict of secret key-value pairs.
        """
        path = Path(self._config.developer_secrets_path).expanduser()
        if not path.exists():
            return {}
        values = dotenv_values(path)
        return {k: v for k, v in values.items() if v is not None}

    def load_infrastructure(self) -> dict[str, str]:
        """Load infrastructure secrets from environment variables.

        Returns:
            Dict of infrastructure secret key-value pairs.
        """
        result: dict[str, str] = {}
        for var in self._config.infrastructure_env_vars:
            val = os.environ.get(var)
            if val is not None:
                result[var] = val
        return result

    def load_credential_files(self) -> dict[str, str]:
        """Load CLI credential files from the secrets directory.

        Only loads files for CLIs in ``required_clis``.

        Returns:
            Dict mapping credential keys to file contents for files that exist
            and are non-empty.
        """
        secrets_dir = Path(self._config.developer_secrets_path).expanduser().parent

        mapping = {
            key: filename
            for cli, (key, filename) in _CLI_CREDENTIAL_FILES.items()
            if cli in self._required_clis
        }

        result: dict[str, str] = {}
        for key, filename in mapping.items():
            path = secrets_dir / filename
            if path.is_file():
                content = path.read_text().strip()
                if content:
                    result[key] = content
        return result

    def build_bundle(
        self,
        project_secrets: dict[str, str] | None = None,
        cloud_secrets: dict[str, str] | None = None,
        developer_overrides: dict[str, str] | None = None,
    ) -> SecretBundle:
        """Build a SecretBundle from all secret sources.

        Args:
            project_secrets: Env vars from the project's .env file.
            cloud_secrets: Secrets fetched from a cloud SecretProvider.
                Merged into the developer scope (overrides dotenv values).
            developer_overrides: Pre-resolved developer secrets (e.g. from
                daemon ``os.environ`` via ``dispatch.required_secrets``).
                When provided, these are used as the developer scope and
                filesystem-based loading is skipped.

        Returns:
            SecretBundle combining developer, project, and infrastructure secrets.
        """
        if developer_overrides is not None:
            developer = dict(developer_overrides)
        else:
            developer = self.load_developer()
            developer.update(self.load_credential_files())
        if cloud_secrets:
            developer.update(cloud_secrets)
        return SecretBundle(
            developer=developer,
            project=project_secrets or {},
            infrastructure=self.load_infrastructure(),
        )
