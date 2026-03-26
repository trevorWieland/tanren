"""Serializable environment handle for persistence between dispatch steps."""

from __future__ import annotations

from typing import Literal

from pydantic import BaseModel, ConfigDict, Field

from tanren_core.adapters.remote_types import VMProvider


class PersistedSSHConfig(BaseModel):
    """Serializable SSH connection configuration.

    Contains everything needed to open a fresh ``SSHConnection`` to a
    previously provisioned VM without holding a live connection object.
    """

    model_config = ConfigDict(extra="forbid", frozen=True)

    host: str = Field(..., description="Remote host address (IP or hostname)")
    user: str = Field(default="root", description="SSH username")
    key_path: str = Field(
        default="~/.ssh/tanren_vm",
        description="Path to the SSH private key file",
    )
    key_content_env: str | None = Field(
        default=None,
        description="Environment variable containing the SSH private key. Re-resolved on recovery.",
    )
    port: int = Field(default=22, ge=1, le=65535, description="SSH port number")
    connect_timeout: int = Field(default=10, ge=1, description="Connection timeout in seconds")
    host_key_policy: Literal["auto_add", "warn", "reject"] = Field(
        default="auto_add",
        description="Host key verification policy",
    )


class PersistedVMInfo(BaseModel):
    """Serializable subset of ``VMHandle``."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    vm_id: str = Field(..., description="Unique VM identifier")
    host: str = Field(..., description="VM host address (IP or hostname)")
    provider: VMProvider = Field(
        default=VMProvider.MANUAL,
        description="Cloud provider managing this VM",
    )
    created_at: str = Field(..., description="ISO 8601 timestamp of VM creation")
    labels: dict[str, str] = Field(default_factory=dict, description="Key-value labels on the VM")
    hourly_cost: float | None = Field(
        default=None,
        ge=0.0,
        description="Estimated hourly cost in USD",
    )


class PersistedDockerConfig(BaseModel):
    """Serializable Docker connection configuration.

    Contains everything needed to reconnect to a previously provisioned
    container without holding a live connection object.
    """

    model_config = ConfigDict(extra="forbid", frozen=True)

    container_id: str = Field(..., description="Docker container ID")
    socket_url: str | None = Field(default=None, description="Docker socket URL (None = default)")


class PersistedEnvironmentHandle(BaseModel):
    """Serializable environment handle persisted in ``StepCompleted`` events.

    Contains everything a subsequent execute or teardown step needs to
    reconstruct a live ``EnvironmentHandle`` (SSH connection, workspace path,
    etc.) without access to the original provision process's memory.
    """

    model_config = ConfigDict(extra="forbid", frozen=True)

    env_id: str = Field(..., description="Unique environment identifier")
    worktree_path: str = Field(
        ...,
        description="Absolute path to the workspace (local or remote)",
    )
    branch: str = Field(..., description="Git branch checked out")
    project: str = Field(..., description="Target project name")

    # Remote fields (all None for local environments)
    vm: PersistedVMInfo | None = Field(
        default=None,
        description="VM info (None for local environments)",
    )
    ssh_config: PersistedSSHConfig | None = Field(
        default=None,
        description="SSH connection config (None for local)",
    )
    workspace_remote_path: str | None = Field(
        default=None,
        description="Remote workspace path on the VM",
    )
    teardown_commands: tuple[str, ...] = Field(
        default_factory=tuple,
        description="Commands to run during teardown",
    )
    profile_name: str = Field(
        default="default",
        description="Name of the resolved environment profile",
    )
    dispatch_id: str | None = Field(
        default=None,
        description="Original dispatch/workflow ID for handle reconstruction",
    )
    provision_timestamp: str = Field(
        ...,
        description="ISO 8601 timestamp when provisioning completed",
    )
    agent_user: str | None = Field(
        default=None,
        description="Agent user for su - wrapping on remote VMs",
    )

    # Docker fields (all None for non-Docker environments)
    docker_config: PersistedDockerConfig | None = Field(
        default=None,
        description="Docker connection config (None for non-Docker environments)",
    )

    # Local fields
    task_env: dict[str, str] = Field(
        default_factory=dict,
        description="Environment variables to inject (local only)",
    )
    preflight_hashes: dict[str, str] = Field(
        default_factory=dict,
        description="File hashes from preflight (local only, for postflight integrity checks)",
    )
    preflight_backups: dict[str, str] = Field(
        default_factory=dict,
        description="File backups from preflight (local only, for postflight reversion)",
    )
