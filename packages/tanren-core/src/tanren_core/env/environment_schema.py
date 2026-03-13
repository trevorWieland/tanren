"""Environment profile parsing from tanren.yml."""

from __future__ import annotations

from collections.abc import Mapping
from enum import StrEnum

from pydantic import BaseModel, ConfigDict, Field


class EnvironmentProfileType(StrEnum):
    """Supported environment profile types."""

    LOCAL = "local"
    REMOTE = "remote"
    DOCKER = "docker"


class ResourceRequirements(BaseModel):
    """Resource requirements for an execution environment."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    cpu: int = Field(default=2, ge=1)
    memory_gb: int = Field(default=4, ge=1)
    gpu: bool = Field(default=False)


class EnvironmentProfile(BaseModel):
    """Parsed environment profile from tanren.yml."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    name: str = Field(...)
    type: EnvironmentProfileType = Field(default=EnvironmentProfileType.LOCAL)
    resources: ResourceRequirements = Field(default_factory=ResourceRequirements)
    setup: tuple[str, ...] = Field(default_factory=tuple)
    teardown: tuple[str, ...] = Field(default_factory=tuple)
    gate_cmd: str = Field(default="make check")
    server_type: str | None = Field(default=None)


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
