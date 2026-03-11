"""Remote execution configuration loaded from remote.yml."""

from __future__ import annotations

import logging
from dataclasses import dataclass, field
from pathlib import Path

import yaml

logger = logging.getLogger(__name__)


@dataclass(frozen=True)
class RemoteSSHConfig:
    """SSH defaults from remote.yml."""

    user: str = "root"
    key_path: str = "~/.ssh/tanren_vm"
    connect_timeout: int = 10


@dataclass(frozen=True)
class RemoteGitConfig:
    """Git auth config from remote.yml."""

    auth: str = "token"
    token_env: str = "GIT_TOKEN"


@dataclass(frozen=True)
class RemoteBootstrapConfig:
    """Bootstrap config from remote.yml."""

    extra_script: str | None = None


@dataclass(frozen=True)
class RemoteSecretsConfig:
    """Secrets config from remote.yml."""

    developer_secrets_path: str = ""


@dataclass(frozen=True)
class RemoteConfig:
    """Full remote execution configuration."""

    execution_mode: str = "remote"
    ssh: RemoteSSHConfig = field(default_factory=RemoteSSHConfig)
    git: RemoteGitConfig = field(default_factory=RemoteGitConfig)
    vms: list[dict] = field(default_factory=list)
    bootstrap: RemoteBootstrapConfig = field(
        default_factory=RemoteBootstrapConfig
    )
    secrets: RemoteSecretsConfig = field(default_factory=RemoteSecretsConfig)
    repos: dict[str, str] = field(default_factory=dict)


def load_remote_config(path: str | Path) -> RemoteConfig:
    """Load and parse remote.yml."""
    path = Path(path)
    if not path.exists():
        raise FileNotFoundError(f"Remote config not found: {path}")

    with open(path) as f:
        data = yaml.safe_load(f) or {}

    ssh_data = data.get("ssh", {})
    ssh = RemoteSSHConfig(
        user=str(ssh_data.get("user", "root")),
        key_path=str(ssh_data.get("key_path", "~/.ssh/tanren_vm")),
        connect_timeout=int(ssh_data.get("connect_timeout", 10)),
    )

    git_data = data.get("git", {})
    git = RemoteGitConfig(
        auth=str(git_data.get("auth", "token")),
        token_env=str(git_data.get("token_env", "GIT_TOKEN")),
    )

    bootstrap_data = data.get("bootstrap", {})
    bootstrap = RemoteBootstrapConfig(
        extra_script=bootstrap_data.get("extra_script"),
    )

    secrets_data = data.get("secrets", {})
    secrets = RemoteSecretsConfig(
        developer_secrets_path=str(
            secrets_data.get("developer_secrets_path", "")
        ),
    )

    vms = data.get("vms", [])
    repos = data.get("repos", {})

    return RemoteConfig(
        execution_mode=str(data.get("execution_mode", "remote")),
        ssh=ssh,
        git=git,
        vms=vms if isinstance(vms, list) else [],
        bootstrap=bootstrap,
        secrets=secrets,
        repos=repos if isinstance(repos, dict) else {},
    )
