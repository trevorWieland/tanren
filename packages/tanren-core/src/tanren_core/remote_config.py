"""Remote execution configuration loaded from remote.yml."""

from __future__ import annotations

from collections.abc import Mapping
from enum import StrEnum
from pathlib import Path
from typing import Literal, cast

import yaml
from pydantic import BaseModel, ConfigDict, Field, JsonValue


class ExecutionMode(StrEnum):
    """Execution mode selector for worker runtime."""

    REMOTE = "remote"
    LOCAL = "local"


class ProvisionerType(StrEnum):
    """Supported VM provisioner backends."""

    MANUAL = "manual"
    HETZNER = "hetzner"
    GCP = "gcp"


class GitAuthMethod(StrEnum):
    """Supported git authentication methods for remote clone/push."""

    TOKEN = "token"  # noqa: S105 — enum value name, not a real password
    SSH = "ssh"


class RemoteSSHConfig(BaseModel):
    """SSH defaults from remote.yml."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    user: str = Field(default="root", description="SSH login user")
    key_path: str = Field(default="~/.ssh/tanren_vm", description="Path to SSH private key")
    port: int = Field(default=22, ge=1, le=65535, description="SSH port number")
    connect_timeout: int = Field(default=10, ge=1, description="SSH connection timeout in seconds")
    host_key_policy: Literal["auto_add", "warn", "reject"] = Field(
        default="auto_add", description="SSH host key verification policy"
    )
    ssh_ready_timeout_secs: int = Field(
        default=300, ge=30, description="Max seconds to wait for SSH readiness after VM boot"
    )


class RemoteGitConfig(BaseModel):
    """Git auth config from remote.yml."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    auth: GitAuthMethod = Field(
        default=GitAuthMethod.TOKEN, description="Git authentication method"
    )
    token_env: str = Field(
        default="GIT_TOKEN", description="Environment variable name containing the git auth token"
    )


class RemoteProvisionerConfig(BaseModel):
    """Provider-agnostic provisioner config from remote.yml."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    type: ProvisionerType = Field(..., description="VM provisioner backend type")
    settings: dict[str, JsonValue] = Field(
        default_factory=dict, description="Provider-specific provisioner settings"
    )


class RemoteBootstrapConfig(BaseModel):
    """Bootstrap config from remote.yml."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    extra_script: str | None = Field(
        default=None, description="Optional shell script to run during VM bootstrap"
    )


class RemoteSecretsConfig(BaseModel):
    """Secrets config from remote.yml."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    developer_secrets_path: str = Field(
        default="", description="Path to the developer secrets.env file"
    )


class RemoteRepoBinding(BaseModel):
    """Repository URL binding for a specific project."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    project: str = Field(..., description="Project name matching the repo")
    repo_url: str = Field(..., description="Git remote URL for the project")
    metadata: dict[str, str] = Field(
        default_factory=dict, description="Arbitrary key-value metadata for the binding"
    )


class RemoteConfig(BaseModel):
    """Full remote execution configuration."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    execution_mode: ExecutionMode = Field(
        default=ExecutionMode.REMOTE, description="Whether to execute locally or on remote VMs"
    )
    ssh: RemoteSSHConfig = Field(
        default_factory=RemoteSSHConfig, description="SSH connection defaults"
    )
    git: RemoteGitConfig = Field(
        default_factory=RemoteGitConfig, description="Git authentication configuration"
    )
    provisioner: RemoteProvisionerConfig = Field(
        ..., description="VM provisioner backend configuration"
    )
    bootstrap: RemoteBootstrapConfig = Field(
        default_factory=RemoteBootstrapConfig, description="VM bootstrap configuration"
    )
    secrets: RemoteSecretsConfig = Field(
        default_factory=RemoteSecretsConfig, description="Secret loading configuration"
    )
    repos: list[RemoteRepoBinding] = Field(
        default_factory=list, description="Project-to-repo URL bindings"
    )

    def repo_url_for(self, project: str) -> str | None:
        """Return configured repo URL for a project name."""
        for binding in self.repos:
            if binding.project == project:
                return binding.repo_url
        return None


def _coerce_repos(
    raw: JsonValue,
) -> list[RemoteRepoBinding]:
    """Coerce repos section into a typed list of bindings.

    Returns:
        List of RemoteRepoBinding instances.
    """
    if isinstance(raw, list):
        bindings: list[RemoteRepoBinding] = [
            RemoteRepoBinding.model_validate(item) for item in raw if isinstance(item, Mapping)
        ]
        return bindings
    if isinstance(raw, Mapping):
        bindings = []
        for project, url in raw.items():
            bindings.append(
                RemoteRepoBinding(
                    project=str(project),
                    repo_url=str(url),
                )
            )
        return bindings
    return []


def _coerce_provisioner(
    raw: JsonValue,
) -> dict[str, JsonValue]:
    """Coerce provisioner section to a plain dict for Pydantic validation.

    Returns:
        Dict with ``type`` and ``settings`` keys.
    """
    if not isinstance(raw, Mapping):
        return {}
    raw_mapping = {str(k): v for k, v in raw.items()}
    settings_raw = raw_mapping.get("settings", {})
    settings: dict[str, JsonValue]
    if isinstance(settings_raw, Mapping):
        settings = {str(k): cast("JsonValue", v) for k, v in settings_raw.items()}
    else:
        settings = {}
    return {
        "type": cast("JsonValue", raw_mapping.get("type")),
        "settings": cast("JsonValue", settings),
    }


def load_remote_config(path: str | Path) -> RemoteConfig:
    """Load and parse remote.yml.

    Returns:
        Validated RemoteConfig.

    Raises:
        FileNotFoundError: If the config file does not exist at the given path.
    """
    path_obj = Path(path)
    if not path_obj.exists():
        raise FileNotFoundError(f"Remote config not found: {path_obj}")

    with open(path_obj) as file_obj:
        data_raw = yaml.safe_load(file_obj) or {}

    if not isinstance(data_raw, Mapping):
        data_raw = {}

    data: dict[str, JsonValue] = dict(data_raw)
    data["repos"] = _coerce_repos(data.get("repos", []))
    data["provisioner"] = _coerce_provisioner(data.get("provisioner", {}))
    return RemoteConfig.model_validate(data)
