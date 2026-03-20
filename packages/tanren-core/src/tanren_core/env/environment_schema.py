"""Environment profile parsing from tanren.yml."""

from __future__ import annotations

import re
from enum import StrEnum
from typing import TYPE_CHECKING

from pydantic import BaseModel, ConfigDict, Field, JsonValue, field_validator

if TYPE_CHECKING:
    from collections.abc import Mapping


class EnvironmentProfileType(StrEnum):
    """Supported environment profile types."""

    LOCAL = "local"
    REMOTE = "remote"
    DOCKER = "docker"


class ResourceRequirements(BaseModel):
    """Resource requirements for an execution environment."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    cpu: int = Field(default=2, ge=1, description="Number of CPU cores required")
    memory_gb: int = Field(default=4, ge=1, description="Memory requirement in gigabytes")
    gpu: bool = Field(default=False, description="Whether a GPU is required")


class McpServerConfig(BaseModel):
    """MCP server configuration for remote CLI environments."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    url: str = Field(..., description="MCP server endpoint URL")
    headers: dict[str, str] = Field(
        default_factory=dict, description="HTTP headers to include in MCP requests"
    )


_MCP_NAME_RE = re.compile(r"^[A-Za-z0-9_-]+$")


class EnvironmentProfile(BaseModel):
    """Parsed environment profile from tanren.yml."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    name: str = Field(..., description="Profile name used for selection")
    type: EnvironmentProfileType = Field(
        default=EnvironmentProfileType.LOCAL, description="Execution environment type"
    )
    resources: ResourceRequirements = Field(
        default_factory=ResourceRequirements, description="Compute resource requirements"
    )
    setup: tuple[str, ...] = Field(
        default_factory=tuple, description="Shell commands to run during environment setup"
    )
    teardown: tuple[str, ...] = Field(
        default_factory=tuple, description="Shell commands to run during environment teardown"
    )
    gate_cmd: str = Field(default="make check", description="Default gate command for this profile")
    server_type: str | None = Field(
        default=None, description="VM server type hint for the provisioner"
    )
    mcp: dict[str, McpServerConfig] = Field(
        default_factory=dict, description="MCP server configurations keyed by server name"
    )

    @field_validator("mcp")
    @classmethod
    def _validate_mcp_names(cls, v: dict[str, McpServerConfig]) -> dict[str, McpServerConfig]:
        for key in v:
            if not _MCP_NAME_RE.match(key):
                raise ValueError(
                    f"MCP server name {key!r} must match [A-Za-z0-9_-]+ "
                    f"(required for TOML bare keys)"
                )
        return v


def parse_environment_profiles(data: Mapping[str, object]) -> dict[str, EnvironmentProfile]:
    """Parse 'environment' section of tanren.yml.

    Always returns at least 'default' (type=local).

    Returns:
        Dict mapping profile names to EnvironmentProfile instances.
    """
    profiles: dict[str, EnvironmentProfile] = {}

    env_section = data.get("environment")
    if isinstance(env_section, dict):
        for raw_name, raw in env_section.items():
            if isinstance(raw, dict):
                name = str(raw_name)
                profiles[name] = EnvironmentProfile.model_validate({"name": name, **raw})

    if "default" not in profiles:
        profiles["default"] = EnvironmentProfile(name="default")

    return profiles


class IssueSourceType(StrEnum):
    """Supported issue source backends."""

    GITHUB = "github"
    LINEAR = "linear"


class IssueSourceConfig(BaseModel):
    """Issue source configuration from tanren.yml."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    type: IssueSourceType = Field(
        default=IssueSourceType.GITHUB, description="Issue tracker backend type"
    )
    settings: dict[str, JsonValue] = Field(
        default_factory=dict, description="Provider-specific issue source settings"
    )


def parse_issue_source(data: Mapping[str, object]) -> IssueSourceConfig | None:
    """Parse 'issue_source' section of tanren.yml.

    Returns:
        IssueSourceConfig if present, None otherwise.
    """
    raw = data.get("issue_source")
    if isinstance(raw, dict):
        return IssueSourceConfig.model_validate(raw)
    return None
