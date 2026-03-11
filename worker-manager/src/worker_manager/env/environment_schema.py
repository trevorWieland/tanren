"""Environment profile parsing from tanren.yml."""

from __future__ import annotations

from dataclasses import dataclass, field


@dataclass(frozen=True)
class ResourceRequirements:
    """Resource requirements for an execution environment."""

    cpu: int = 2
    memory_gb: int = 4
    gpu: bool = False


@dataclass(frozen=True)
class EnvironmentProfile:
    """Parsed environment profile from tanren.yml."""

    name: str
    type: str = "local"
    resources: ResourceRequirements = field(default_factory=ResourceRequirements)
    setup: tuple[str, ...] = ()
    teardown: tuple[str, ...] = ()
    gate_cmd: str = "make check"


def parse_environment_profiles(data: dict) -> dict[str, EnvironmentProfile]:
    """Parse 'environment' section of tanren.yml.

    Always returns at least 'default' (type=local).
    """
    profiles: dict[str, EnvironmentProfile] = {}

    env_section = data.get("environment", {})
    if not isinstance(env_section, dict):
        env_section = {}

    for name, raw in env_section.items():
        if not isinstance(raw, dict):
            continue

        raw_resources = raw.get("resources", {})
        if not isinstance(raw_resources, dict):
            raw_resources = {}

        resources = ResourceRequirements(
            cpu=int(raw_resources.get("cpu", 2)),
            memory_gb=int(raw_resources.get("memory_gb", 4)),
            gpu=bool(raw_resources.get("gpu", False)),
        )

        setup_raw = raw.get("setup", [])
        teardown_raw = raw.get("teardown", [])

        profiles[name] = EnvironmentProfile(
            name=name,
            type=str(raw.get("type", "local")),
            resources=resources,
            setup=tuple(setup_raw) if isinstance(setup_raw, list) else (),
            teardown=tuple(teardown_raw) if isinstance(teardown_raw, list) else (),
            gate_cmd=str(raw.get("gate_cmd", "make check")),
        )

    # Always provide a default profile
    if "default" not in profiles:
        profiles["default"] = EnvironmentProfile(name="default")

    return profiles
