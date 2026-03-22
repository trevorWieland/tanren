"""Shared builder for SSHExecutionEnvironment used by daemon and CLI."""

from __future__ import annotations

import logging
import os
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import asyncpg

    from tanren_core.adapters.protocols import VMStateStore
    from tanren_core.worker_config import WorkerConfig

from tanren_core.adapters.credentials import providers_for_clis
from tanren_core.adapters.git_workspace import GitAuthConfig, GitWorkspaceManager
from tanren_core.adapters.manual_vm import ManualProvisionerSettings, ManualVMProvisioner
from tanren_core.adapters.remote_runner import RemoteAgentRunner
from tanren_core.adapters.remote_types import VMProvider
from tanren_core.adapters.ssh import SSHConfig
from tanren_core.adapters.ssh_environment import SSHExecutionEnvironment
from tanren_core.adapters.ubuntu_bootstrap import UbuntuBootstrapper
from tanren_core.remote_config import ProvisionerType, load_remote_config
from tanren_core.roles_config import load_roles_config
from tanren_core.secrets import SecretConfig, SecretLoader

logger = logging.getLogger(__name__)

_AGENT_USER = "tanren"


def build_ssh_execution_environment(
    config: WorkerConfig,
    pool: asyncpg.Pool | None = None,
) -> tuple[SSHExecutionEnvironment, VMStateStore]:
    """Construct an SSHExecutionEnvironment from config.

    Returns:
        Tuple of (SSHExecutionEnvironment, SqliteVMStateStore).

    Raises:
        ValueError: If the provisioner type in remote.yml is unsupported.
    """
    if config.remote_config_path is None:
        raise ValueError("remote_config_path is required to build SSH execution environment")
    remote_cfg = load_remote_config(config.remote_config_path)

    # Load roles config to determine required CLIs
    roles = load_roles_config(config.roles_config_path)
    required_clis = roles.required_clis()

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

    # Read extra bootstrap script if configured
    extra_script = None
    if remote_cfg.bootstrap.extra_script:
        script_path = Path(remote_cfg.bootstrap.extra_script).expanduser()
        if not script_path.is_absolute():
            config_dir = Path(config.remote_config_path).resolve().parent
            script_path = config_dir / script_path
        if script_path.exists():
            extra_script = script_path.read_text()
        else:
            logger.warning("Bootstrap extra script not found: %s", script_path)

    secret_config = SecretConfig(
        developer_secrets_path=(
            remote_cfg.secrets.developer_secrets_path or SecretConfig().developer_secrets_path
        ),
    )
    secret_loader = SecretLoader(secret_config, required_clis=required_clis)
    secret_loader.autoload_into_env(override=False)

    token = os.environ.get(remote_cfg.git.token_env, "")
    git_auth = GitAuthConfig(
        auth_method=remote_cfg.git.auth,
        token=token or None,
    )

    if remote_cfg.provisioner.type == ProvisionerType.MANUAL:
        manual_settings = ManualProvisionerSettings.from_settings(remote_cfg.provisioner.settings)
        vm_provisioner = ManualVMProvisioner(list(manual_settings.vms), state_store)
        provider = VMProvider.MANUAL
    elif remote_cfg.provisioner.type == ProvisionerType.HETZNER:
        from tanren_core.adapters.hetzner_vm import (  # noqa: PLC0415 — optional dep
            HetznerProvisionerSettings,
            HetznerVMProvisioner,
        )

        hetzner_settings = HetznerProvisionerSettings.from_settings(remote_cfg.provisioner.settings)
        vm_provisioner = HetznerVMProvisioner(hetzner_settings)
        provider = VMProvider.HETZNER
    elif remote_cfg.provisioner.type == ProvisionerType.GCP:
        from tanren_core.adapters.gcp_vm import (  # noqa: PLC0415 — optional dep
            GCPProvisionerSettings,
            GCPVMProvisioner,
        )

        gcp_settings = GCPProvisionerSettings.from_settings(remote_cfg.provisioner.settings)
        vm_provisioner = GCPVMProvisioner(gcp_settings)
        provider = VMProvider.GCP
    else:
        raise ValueError(f"Unsupported provisioner type: {remote_cfg.provisioner.type}")

    env = SSHExecutionEnvironment(
        vm_provisioner=vm_provisioner,
        bootstrapper=UbuntuBootstrapper(
            required_clis=required_clis,
            extra_script=extra_script,
        ),
        workspace_mgr=GitWorkspaceManager(git_auth),
        runner=RemoteAgentRunner(run_as_user=_AGENT_USER),
        state_store=state_store,
        secret_loader=secret_loader,
        ssh_config_defaults=ssh_defaults,
        repo_urls={binding.project: binding.repo_url for binding in remote_cfg.repos},
        provider=provider,
        ssh_ready_timeout_secs=remote_cfg.ssh.ssh_ready_timeout_secs,
        credential_providers=providers_for_clis(required_clis),
        agent_user=_AGENT_USER,
    )

    return env, state_store
