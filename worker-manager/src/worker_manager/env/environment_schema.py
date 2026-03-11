"""Environment profile parsing from tanren.yml."""

from __future__ import annotations

from enum import StrEnum
from typing import cast

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


def _coerce_str_tuple(raw: object) -> tuple[str, ...]:
    if not isinstance(raw, list):
        return ()
    return tuple(str(v) for v in raw)


def _coerce_environment_type(raw: object) -> EnvironmentProfileType:
    if isinstance(raw, EnvironmentProfileType):
        return raw
    raw_value = str(raw).strip()
    if not raw_value:
        return EnvironmentProfileType.LOCAL
    return EnvironmentProfileType(raw_value)


def _coerce_int(raw: object, default: int) -> int:
    if isinstance(raw, int):
        return raw
    if isinstance(raw, str):
        try:
            return int(raw)
        except ValueError:
            return default
    return default


def parse_environment_profiles(data: dict[str, object]) -> dict[str, EnvironmentProfile]:
    """Parse 'environment' section of tanren.yml.

    Always returns at least 'default' (type=local).
    """
    profiles: dict[str, EnvironmentProfile] = {}

    env_section_raw = data.get("environment", {})
    env_section: dict[str, object] = (
        cast(dict[str, object], env_section_raw) if isinstance(env_section_raw, dict) else {}
    )

    for raw_name, raw in env_section.items():
        name = str(raw_name)
        if not isinstance(raw, dict):
            continue

        raw_dict = cast(dict[str, object], raw)
        raw_resources_data = raw_dict.get("resources", {})
        raw_resources: dict[str, object] = (
            cast(dict[str, object], raw_resources_data)
            if isinstance(raw_resources_data, dict)
            else {}
        )

        resources = ResourceRequirements(
            cpu=_coerce_int(raw_resources.get("cpu", 2), 2),
            memory_gb=_coerce_int(raw_resources.get("memory_gb", 4), 4),
            gpu=bool(raw_resources.get("gpu", False)),
        )

        setup_raw = raw_dict.get("setup", [])
        teardown_raw = raw_dict.get("teardown", [])

        profiles[name] = EnvironmentProfile(
            name=name,
            type=_coerce_environment_type(raw_dict.get("type", EnvironmentProfileType.LOCAL.value)),
            resources=resources,
            setup=_coerce_str_tuple(setup_raw),
            teardown=_coerce_str_tuple(teardown_raw),
            gate_cmd=str(raw_dict.get("gate_cmd", "make check")),
            server_type=(
                str(raw_dict["server_type"])
                if "server_type" in raw_dict and raw_dict["server_type"] is not None
                else None
            ),
        )

    # Always provide a default profile
    if "default" not in profiles:
        profiles["default"] = EnvironmentProfile(name="default")

    return profiles
