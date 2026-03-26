"""Environment profile parsing from tanren.yml."""

from __future__ import annotations

import re
from enum import StrEnum
from typing import TYPE_CHECKING, Literal

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


class SSHDefaults(BaseModel):
    """SSH connection defaults carried in the dispatch."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    user: str = Field(default="root", description="SSH login user")
    key_path: str = Field(default="~/.ssh/tanren_vm", description="Path to SSH private key")
    key_content_env: str | None = Field(
        default=None,
        description="Env var holding the SSH private key; overrides key_path.",
    )
    port: int = Field(default=22, ge=1, le=65535, description="SSH port number")
    connect_timeout: int = Field(default=10, ge=1, description="SSH connection timeout in seconds")
    host_key_policy: Literal["auto_add", "warn", "reject"] = Field(
        default="auto_add", description="SSH host key verification policy"
    )
    ssh_ready_timeout_secs: int = Field(
        default=300, ge=30, description="Max seconds to wait for SSH readiness"
    )


class DispatchGitConfig(BaseModel):
    """Git auth config carried in the dispatch."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    auth_method: str = Field(
        default="token", description="Git authentication method (token or ssh)"
    )
    token_env: str = Field(
        default="GIT_TOKEN", description="Environment variable name for git auth token"
    )


class DispatchProvisionerConfig(BaseModel):
    """Provisioner config carried in the dispatch."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    type: str = Field(..., description="VM provisioner backend (hetzner, gcp, manual)")
    settings: dict[str, JsonValue] = Field(
        default_factory=dict, description="Provider-specific settings"
    )


class DockerExecutionConfig(BaseModel):
    """Docker execution config carried in the dispatch — everything the daemon needs."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    image: str = Field(default="ubuntu:24.04", description="Docker image to use")
    socket_url: str | None = Field(
        default=None, description="Docker socket URL (None = default /var/run/docker.sock)"
    )
    network: str | None = Field(default=None, description="Docker network to attach container to")
    extra_volumes: tuple[str, ...] = Field(
        default_factory=tuple, description="Additional volume mounts (host:container format)"
    )
    extra_env: dict[str, str] = Field(
        default_factory=dict, description="Additional environment variables for the container"
    )
    repo_url: str = Field(..., description="Git clone URL for this project")
    required_clis: tuple[str, ...] = Field(
        default_factory=tuple, description="CLIs needed for bootstrap"
    )
    bootstrap_extra_script: str | None = Field(
        default=None, description="Inline bootstrap script content"
    )
    agent_user: str = Field(default="tanren", description="Unprivileged user in the container")
    git: DispatchGitConfig = Field(default_factory=DispatchGitConfig, description="Git auth config")


class RemoteExecutionConfig(BaseModel):
    """Remote execution config carried in the dispatch — everything the daemon needs."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    ssh: SSHDefaults = Field(default_factory=SSHDefaults, description="SSH connection defaults")
    git: DispatchGitConfig = Field(default_factory=DispatchGitConfig, description="Git auth config")
    provisioner: DispatchProvisionerConfig = Field(..., description="VM provisioner config")
    repo_url: str = Field(..., description="Git clone URL for this project")
    required_clis: tuple[str, ...] = Field(
        default_factory=tuple, description="CLIs needed for bootstrap"
    )
    bootstrap_extra_script: str | None = Field(
        default=None, description="Inline bootstrap script content"
    )
    agent_user: str = Field(default="tanren", description="Unprivileged user on remote VM")


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
    task_gate_cmd: str | None = Field(
        default=None,
        description=(
            "Gate command for task-scoped phases (do-task, gate, audit-task);"
            " falls back to gate_cmd"
        ),
    )
    spec_gate_cmd: str | None = Field(
        default=None,
        description="Gate command for spec-scoped phases (run-demo, audit-spec)."
        " Falls back to gate_cmd",
    )
    server_type: str | None = Field(
        default=None, description="VM server type hint for the provisioner"
    )
    mcp: dict[str, McpServerConfig] = Field(
        default_factory=dict, description="MCP server configurations keyed by server name"
    )
    remote_config: RemoteExecutionConfig | None = Field(
        default=None, description="Remote execution config (required when type is remote)"
    )
    docker_config: DockerExecutionConfig | None = Field(
        default=None, description="Docker execution config (required when type is docker)"
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


def parse_environment_profiles(data: Mapping[str, JsonValue]) -> dict[str, EnvironmentProfile]:
    """Parse 'environment' section of tanren.yml.

    Returns:
        Dict mapping profile names to EnvironmentProfile instances.
        May be empty if no profiles are defined.
    """
    profiles: dict[str, EnvironmentProfile] = {}

    env_section = data.get("environment")
    if isinstance(env_section, dict):
        for raw_name, raw in env_section.items():
            if isinstance(raw, dict):
                name = str(raw_name)
                profiles[name] = EnvironmentProfile.model_validate({"name": name, **raw})

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


def parse_issue_source(data: Mapping[str, JsonValue]) -> IssueSourceConfig | None:
    """Parse 'issue_source' section of tanren.yml.

    Returns:
        IssueSourceConfig if present, None otherwise.
    """
    raw = data.get("issue_source")
    if isinstance(raw, dict):
        return IssueSourceConfig.model_validate(raw)
    return None
