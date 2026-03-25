"""Shared builders for execution environments used by daemon and CLI.

``build_execution_environment`` is the profile-driven factory that dispatches
on ``EnvironmentProfileType``.  ``build_ssh_execution_environment`` is the
remote-specific builder retained for callers that already know the profile
is remote.
"""

from __future__ import annotations

import logging
import os
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import asyncpg

    from tanren_core.adapters.protocols import ExecutionEnvironment, VMStateStore
    from tanren_core.worker_config import WorkerConfig

from tanren_core.adapters.credentials import providers_for_clis
from tanren_core.adapters.git_workspace import GitAuthConfig, GitWorkspaceManager
from tanren_core.adapters.manual_vm import ManualProvisionerSettings, ManualVMProvisioner
from tanren_core.adapters.remote_runner import RemoteAgentRunner
from tanren_core.adapters.remote_types import VMProvider
from tanren_core.adapters.ssh import SSHConfig
from tanren_core.adapters.ssh_environment import SSHExecutionEnvironment
from tanren_core.adapters.ubuntu_bootstrap import UbuntuBootstrapper
from tanren_core.env.environment_schema import (
    EnvironmentProfile,
    EnvironmentProfileType,
    RemoteExecutionConfig,
)
from tanren_core.remote_config import GitAuthMethod, ProvisionerType
from tanren_core.schemas import Cli
from tanren_core.secrets import SecretConfig, SecretLoader

logger = logging.getLogger(__name__)

# ── Adapter requirement registry ─────────────────────────────────────────────
# Single source of truth for per-adapter env vars and packages.
# Add entries here when implementing a new provisioner adapter.

_ADAPTER_REQUIREMENTS: dict[ProvisionerType, list[tuple[str, str]]] = {
    ProvisionerType.HETZNER: [
        ("HCLOUD_TOKEN", "Hetzner Cloud API token"),
    ],
    ProvisionerType.GCP: [
        ("GCP_SSH_PUBLIC_KEY", "SSH public key for GCP VM access"),
    ],
}

_ADAPTER_PACKAGES: dict[ProvisionerType, tuple[str, str]] = {
    ProvisionerType.HETZNER: ("hcloud", "hetzner"),
    ProvisionerType.GCP: ("google.cloud.compute_v1", "gcp"),
}


def validate_provisioner_requirements(provisioner_type: ProvisionerType | str) -> None:
    """Validate env vars and packages for the configured provisioner.

    Checks that the required Python package is importable and that all
    required environment variables are set.  Called at startup before
    constructing the provisioner so the daemon fails fast with a clear
    message listing every missing requirement.

    Args:
        provisioner_type: Provisioner type as ProvisionerType enum or string.

    Raises:
        ValueError: With a message listing all missing requirements.
    """
    ptype = (
        ProvisionerType(provisioner_type) if isinstance(provisioner_type, str) else provisioner_type
    )
    errors: list[str] = []

    if ptype in _ADAPTER_PACKAGES:
        module_name, extra_name = _ADAPTER_PACKAGES[ptype]
        try:
            __import__(module_name)
        except ImportError:
            errors.append(
                f"Python package '{module_name}' is not installed. Build with: --extra {extra_name}"
            )

    for env_var, description in _ADAPTER_REQUIREMENTS.get(ptype, []):
        if not os.environ.get(env_var, "").strip():
            errors.append(f"Missing env var {env_var} ({description})")

    if errors:
        raise ValueError(
            f"Adapter '{ptype}' configuration errors:\n" + "\n".join(f"  - {e}" for e in errors)
        )


def build_ssh_execution_environment(
    config: WorkerConfig,
    remote_cfg: RemoteExecutionConfig,
    pool: asyncpg.Pool | None = None,
) -> tuple[SSHExecutionEnvironment, VMStateStore]:
    """Construct an SSHExecutionEnvironment from dispatch-carried config.

    Args:
        config: Worker operational config (data_dir, etc.).
        remote_cfg: Remote execution config carried in the dispatch/profile.
        pool: Optional asyncpg pool for Postgres-backed VM state.

    Returns:
        Tuple of (SSHExecutionEnvironment, VMStateStore).

    Raises:
        ValueError: If the provisioner type is unsupported.
    """
    # Convert required_clis strings to Cli enum set
    required_clis = frozenset(Cli(c) for c in remote_cfg.required_clis)

    agent_user = remote_cfg.agent_user

    ssh_defaults = SSHConfig(
        host="",  # placeholder — overridden per VM
        user=remote_cfg.ssh.user,
        key_path=remote_cfg.ssh.key_path,
        port=remote_cfg.ssh.port,
        connect_timeout=remote_cfg.ssh.connect_timeout,
        host_key_policy=remote_cfg.ssh.host_key_policy,
    )

    if pool is not None:
        from tanren_core.adapters.postgres_vm_state import (  # noqa: PLC0415 — conditional import based on configuration
            PostgresVMStateStore,
        )

        state_store: VMStateStore = PostgresVMStateStore(pool)
    else:
        from tanren_core.adapters.sqlite_vm_state import (  # noqa: PLC0415 — conditional import based on configuration
            SqliteVMStateStore,
        )

        state_store = SqliteVMStateStore(f"{config.data_dir}/vm-state.db")

    # Bootstrap extra script is carried inline (already resolved by CLI)
    extra_script = remote_cfg.bootstrap_extra_script

    # Daemon uses its own secrets path (not from dispatch)
    secret_config = SecretConfig()
    secret_loader = SecretLoader(secret_config, required_clis=required_clis)
    secret_loader.autoload_into_env(override=False)

    # Resolve git token from daemon's own environment
    token = os.environ.get(remote_cfg.git.token_env, "")
    git_auth = GitAuthConfig(
        auth_method=GitAuthMethod(remote_cfg.git.auth_method),
        token=token or None,
    )

    provisioner_type = ProvisionerType(remote_cfg.provisioner.type)
    validate_provisioner_requirements(provisioner_type)

    if provisioner_type == ProvisionerType.MANUAL:
        manual_settings = ManualProvisionerSettings.from_settings(remote_cfg.provisioner.settings)
        vm_provisioner = ManualVMProvisioner(list(manual_settings.vms), state_store)
        provider = VMProvider.MANUAL
    elif provisioner_type == ProvisionerType.HETZNER:
        from tanren_core.adapters.hetzner_vm import (  # noqa: PLC0415 — optional dep
            HetznerProvisionerSettings,
            HetznerVMProvisioner,
        )

        hetzner_settings = HetznerProvisionerSettings.from_settings(remote_cfg.provisioner.settings)
        vm_provisioner = HetznerVMProvisioner(hetzner_settings)
        provider = VMProvider.HETZNER
    elif provisioner_type == ProvisionerType.GCP:
        from tanren_core.adapters.gcp_vm import (  # noqa: PLC0415 — optional dep
            GCPProvisionerSettings,
            GCPVMProvisioner,
        )

        gcp_settings = GCPProvisionerSettings.from_settings(remote_cfg.provisioner.settings)
        vm_provisioner = GCPVMProvisioner(gcp_settings)
        provider = VMProvider.GCP
    else:
        raise ValueError(f"Unsupported provisioner type: {provisioner_type}")

    env = SSHExecutionEnvironment(
        vm_provisioner=vm_provisioner,
        bootstrapper=UbuntuBootstrapper(
            required_clis=required_clis,
            extra_script=extra_script,
        ),
        workspace_mgr=GitWorkspaceManager(git_auth),
        runner=RemoteAgentRunner(run_as_user=agent_user),
        state_store=state_store,
        secret_loader=secret_loader,
        ssh_config_defaults=ssh_defaults,
        repo_urls={"__dispatch__": remote_cfg.repo_url},
        provider=provider,
        ssh_ready_timeout_secs=remote_cfg.ssh.ssh_ready_timeout_secs,
        credential_providers=providers_for_clis(required_clis),
        agent_user=agent_user,
    )

    return env, state_store


# ── Profile-driven factory ───────────────────────────────────────────────


def _build_local(
    config: WorkerConfig,
) -> tuple[ExecutionEnvironment, None]:
    """Construct a local execution environment with default adapters.

    Returns:
        Tuple of (LocalExecutionEnvironment, None) — no VM state store needed.
    """
    from tanren_core.adapters.dotenv_validator import DotenvEnvValidator  # noqa: PLC0415
    from tanren_core.adapters.git_postflight import GitPostflightRunner  # noqa: PLC0415
    from tanren_core.adapters.git_preflight import GitPreflightRunner  # noqa: PLC0415
    from tanren_core.adapters.git_worktree import GitWorktreeManager  # noqa: PLC0415
    from tanren_core.adapters.local_environment import LocalExecutionEnvironment  # noqa: PLC0415
    from tanren_core.adapters.subprocess_spawner import SubprocessSpawner  # noqa: PLC0415

    env = LocalExecutionEnvironment(
        env_validator=DotenvEnvValidator(),
        preflight=GitPreflightRunner(),
        postflight=GitPostflightRunner(),
        spawner=SubprocessSpawner(),
        worktree_mgr=GitWorktreeManager(),
        config=config,
    )
    return env, None


def build_execution_environment(
    config: WorkerConfig,
    profile: EnvironmentProfile,
    pool: asyncpg.Pool | None = None,
) -> tuple[ExecutionEnvironment, VMStateStore | None]:
    """Build the execution environment for a given profile.

    Dispatches on ``profile.type``. No fallback behaviour — missing
    configuration raises immediately.

    Args:
        config: Worker configuration.
        profile: Resolved environment profile from tanren.yml.
        pool: Optional asyncpg pool for Postgres-backed VM state (remote only).

    Returns:
        Tuple of (ExecutionEnvironment, VMStateStore | None).

    Raises:
        ValueError: If required config for the profile type is missing.
        NotImplementedError: If the profile type is not yet supported.
    """
    if profile.type == EnvironmentProfileType.LOCAL:
        return _build_local(config)
    elif profile.type == EnvironmentProfileType.REMOTE:
        if profile.remote_config is None:
            raise ValueError(f"remote_config is required for remote profile '{profile.name}'")
        return build_ssh_execution_environment(config, profile.remote_config, pool=pool)
    elif profile.type == EnvironmentProfileType.DOCKER:
        raise NotImplementedError("Docker execution not yet supported")
    else:
        raise ValueError(f"Unknown profile type: {profile.type}")
