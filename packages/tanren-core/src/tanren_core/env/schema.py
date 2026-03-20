"""Pydantic models for the tanren.yml env block."""

from enum import StrEnum
from typing import cast

from pydantic import BaseModel, ConfigDict, Field, model_validator


class OnMissing(StrEnum):
    """Policy for handling missing required environment variables."""

    ERROR = "error"
    WARN = "warn"
    PROMPT = "prompt"


class RequiredEnvVar(BaseModel):
    """Required environment variable declaration."""

    model_config = ConfigDict(extra="forbid")

    key: str = Field(...)
    description: str = Field(default="")
    pattern: str | None = Field(default=None, description="Regex for re.fullmatch()")
    hint: str = Field(default="")
    source: str | None = Field(
        default=None,
        description="Secret source, e.g. 'secret:my-api-key' to resolve via SecretProvider",
    )


class OptionalEnvVar(BaseModel):
    """Optional environment variable declaration."""

    model_config = ConfigDict(extra="forbid")

    key: str = Field(...)
    description: str = Field(default="")
    pattern: str | None = Field(default=None)
    default: str | None = Field(default=None)
    source: str | None = Field(
        default=None,
        description="Secret source, e.g. 'secret:my-api-key' to resolve via SecretProvider",
    )


class SecretsProviderType(StrEnum):
    """Supported secret storage backends."""

    DOTENV = "dotenv"
    GCP = "gcp"


class SecretsConfig(BaseModel):
    """Configuration for the project's secret storage backend."""

    model_config = ConfigDict(extra="forbid")

    provider: SecretsProviderType = Field(default=SecretsProviderType.DOTENV)
    settings: dict[str, str] = Field(default_factory=dict)


class EnvBlock(BaseModel):
    """Environment policy and variable requirements for a project."""

    model_config = ConfigDict(extra="forbid")

    on_missing: OnMissing = Field(default=OnMissing.ERROR)
    required: list[RequiredEnvVar] = Field(default_factory=list)
    optional: list[OptionalEnvVar] = Field(default_factory=list)


class TanrenConfig(BaseModel):
    """Top-level tanren.yml model needed by worker-manager env tooling."""

    model_config = ConfigDict(extra="forbid")

    version: str = Field(...)
    profile: str = Field(...)
    installed: str = Field(...)
    env: EnvBlock | None = Field(default=None)
    secrets: SecretsConfig | None = Field(default=None)
    # Consumed by runtime environment selection logic outside env tooling.
    environment: dict[str, object] | None = Field(default=None)
    issue_source: dict[str, object] | None = Field(default=None)

    @model_validator(mode="before")
    @classmethod
    def _coerce_installed(cls, value: object) -> object:
        # YAML can parse bare dates (e.g., 2026-01-01) as datetime.date.
        if isinstance(value, dict) and "installed" in value:
            value_dict = cast(dict[str, object], value)
            return {**value_dict, "installed": str(value_dict.get("installed"))}
        return value
