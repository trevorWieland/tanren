"""Remote execution configuration loaded from remote.yml."""

from __future__ import annotations

from collections.abc import Mapping
from enum import StrEnum
from pathlib import Path

import yaml
from pydantic import AliasChoices, BaseModel, ConfigDict, Field


class ExecutionMode(StrEnum):
    """Execution mode selector for worker runtime."""

    REMOTE = "remote"
    LOCAL = "local"


class GitAuthMethod(StrEnum):
    """Supported git authentication methods for remote clone/push."""

    TOKEN = "token"
    SSH = "ssh"


class RemoteSSHConfig(BaseModel):
    """SSH defaults from remote.yml."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    user: str = Field(default="root")
    key_path: str = Field(default="~/.ssh/tanren_vm")
    port: int = Field(default=22, ge=1, le=65535)
    connect_timeout: int = Field(default=10, ge=1)


class RemoteGitConfig(BaseModel):
    """Git auth config from remote.yml."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    auth: GitAuthMethod = Field(default=GitAuthMethod.TOKEN)
    token_env: str = Field(default="GIT_TOKEN")


class RemoteBootstrapConfig(BaseModel):
    """Bootstrap config from remote.yml."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    extra_script: str | None = Field(default=None)


class RemoteSecretsConfig(BaseModel):
    """Secrets config from remote.yml."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    developer_secrets_path: str = Field(default="")


class RemoteVMConfig(BaseModel):
    """Typed VM entry for remote execution pools."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    vm_id: str = Field(..., validation_alias=AliasChoices("vm_id", "id"))
    host: str = Field(...)
    provider: str = Field(default="manual", min_length=1)
    labels: dict[str, str] = Field(default_factory=dict)
    metadata: dict[str, str] = Field(default_factory=dict)
    hourly_cost: float | None = Field(default=None, ge=0.0)


class RemoteRepoBinding(BaseModel):
    """Repository URL binding for a specific project."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    project: str = Field(...)
    repo_url: str = Field(...)
    metadata: dict[str, str] = Field(default_factory=dict)


class RemoteConfig(BaseModel):
    """Full remote execution configuration."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    execution_mode: ExecutionMode = Field(default=ExecutionMode.REMOTE)
    ssh: RemoteSSHConfig = Field(default_factory=RemoteSSHConfig)
    git: RemoteGitConfig = Field(default_factory=RemoteGitConfig)
    vms: list[RemoteVMConfig] = Field(default_factory=list)
    bootstrap: RemoteBootstrapConfig = Field(default_factory=RemoteBootstrapConfig)
    secrets: RemoteSecretsConfig = Field(default_factory=RemoteSecretsConfig)
    repos: list[RemoteRepoBinding] = Field(default_factory=list)

    def repo_url_for(self, project: str) -> str | None:
        """Return configured repo URL for a project name."""
        for binding in self.repos:
            if binding.project == project:
                return binding.repo_url
        return None


def _coerce_repos(raw: object) -> list[RemoteRepoBinding]:
    """Coerce repos section into a typed list of bindings."""
    if isinstance(raw, list):
        bindings: list[RemoteRepoBinding] = []
        for item in raw:
            if isinstance(item, Mapping):
                bindings.append(RemoteRepoBinding.model_validate(item))
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


def load_remote_config(path: str | Path) -> RemoteConfig:
    """Load and parse remote.yml."""
    path_obj = Path(path)
    if not path_obj.exists():
        raise FileNotFoundError(f"Remote config not found: {path_obj}")

    with open(path_obj) as file_obj:
        data_raw = yaml.safe_load(file_obj) or {}

    if not isinstance(data_raw, Mapping):
        data_raw = {}

    data: dict[str, object] = dict(data_raw)
    data["repos"] = _coerce_repos(data.get("repos", []))
    return RemoteConfig.model_validate(data)
