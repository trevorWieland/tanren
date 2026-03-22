"""Pydantic models for the tanren.yml env block."""

from enum import StrEnum
from typing import cast

from pydantic import BaseModel, ConfigDict, Field, JsonValue, model_validator


class OnMissing(StrEnum):
    """Policy for handling missing required environment variables."""

    ERROR = "error"
    WARN = "warn"
    PROMPT = "prompt"


class RequiredEnvVar(BaseModel):
    """Required environment variable declaration."""

    model_config = ConfigDict(extra="forbid")

    key: str = Field(..., description="Environment variable name")
    description: str = Field(default="", description="Human-readable purpose of this variable")
    pattern: str | None = Field(default=None, description="Regex for re.fullmatch()")
    hint: str = Field(default="", description="User-facing hint for how to set this variable")
    source: str | None = Field(
        default=None,
        description="Secret source, e.g. 'secret:my-api-key' to resolve via SecretProvider",
    )


class OptionalEnvVar(BaseModel):
    """Optional environment variable declaration."""

    model_config = ConfigDict(extra="forbid")

    key: str = Field(..., description="Environment variable name")
    description: str = Field(default="", description="Human-readable purpose of this variable")
    pattern: str | None = Field(default=None, description="Regex pattern for value validation")
    default: str | None = Field(default=None, description="Default value if variable is unset")
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

    provider: SecretsProviderType = Field(
        default=SecretsProviderType.DOTENV, description="Secret storage backend type"
    )
    settings: dict[str, str] = Field(
        default_factory=dict, description="Provider-specific configuration settings"
    )


class EnvBlock(BaseModel):
    """Environment policy and variable requirements for a project."""

    model_config = ConfigDict(extra="forbid")

    on_missing: OnMissing = Field(
        default=OnMissing.ERROR, description="Policy when a required variable is missing"
    )
    required: list[RequiredEnvVar] = Field(
        default_factory=list, description="Required environment variable declarations"
    )
    optional: list[OptionalEnvVar] = Field(
        default_factory=list, description="Optional environment variable declarations"
    )


class TanrenConfig(BaseModel):
    """Top-level tanren.yml model needed by worker-manager env tooling."""

    model_config = ConfigDict(extra="forbid")

    version: str = Field(..., description="tanren.yml schema version")
    profile: str = Field(..., description="Active environment profile name")
    installed: str = Field(..., description="Date or version when tanren was installed")
    env: EnvBlock | None = Field(
        default=None, description="Environment variable policy and declarations"
    )
    secrets: SecretsConfig | None = Field(
        default=None, description="Secret storage backend configuration"
    )
    # Consumed by runtime environment selection logic outside env tooling.
    environment: dict[str, JsonValue] | None = Field(
        default=None, description="Raw environment profile definitions"
    )
    issue_source: dict[str, JsonValue] | None = Field(
        default=None, description="Raw issue source configuration"
    )

    @model_validator(mode="before")
    @classmethod
    def _coerce_installed(cls, value: object) -> object:
        # YAML can parse bare dates (e.g., 2026-01-01) as datetime.date.
        if isinstance(value, dict) and "installed" in value:
            value_dict = cast("dict[str, object]", value)
            return {**value_dict, "installed": str(value_dict.get("installed"))}
        return value
