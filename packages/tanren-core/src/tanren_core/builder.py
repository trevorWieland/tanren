"""Shared builder for SSHExecutionEnvironment.

Extracted from WorkerManager._build_remote_env() so that both the CLI
and the API can construct an SSH execution environment without
instantiating a full WorkerManager.
"""

from __future__ import annotations

import logging
import os
from pathlib import Path

from tanren_core.adapters.credentials import providers_for_clis
from tanren_core.adapters.git_workspace import GitAuthConfig, GitWorkspaceManager
from tanren_core.adapters.manual_vm import ManualProvisionerSettings, ManualVMProvisioner
from tanren_core.adapters.protocols import EventEmitter
from tanren_core.adapters.remote_runner import RemoteAgentRunner
from tanren_core.adapters.remote_types import VMProvider
from tanren_core.adapters.sqlite_vm_state import SqliteVMStateStore
from tanren_core.adapters.ssh import SSHConfig
from tanren_core.adapters.ssh_environment import SSHExecutionEnvironment
from tanren_core.adapters.ubuntu_bootstrap import UbuntuBootstrapper
from tanren_core.config import Config
from tanren_core.remote_config import ProvisionerType, load_remote_config
from tanren_core.roles_config import load_roles_config
from tanren_core.secrets import SecretConfig, SecretLoader

logger = logging.getLogger(__name__)

_AGENT_USER = "tanren"


def build_ssh_execution_environment(
    config: Config,
    emitter: EventEmitter,
) -> tuple[SSHExecutionEnvironment, SqliteVMStateStore]:
    """Construct an SSHExecutionEnvironment from config and emitter.

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
        from tanren_core.adapters.hetzner_vm import (  # noqa: PLC0415
            HetznerProvisionerSettings,
            HetznerVMProvisioner,
        )

        hetzner_settings = HetznerProvisionerSettings.from_settings(remote_cfg.provisioner.settings)
        vm_provisioner = HetznerVMProvisioner(hetzner_settings)
        provider = VMProvider.HETZNER
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
        emitter=emitter,
        ssh_config_defaults=ssh_defaults,
        repo_urls={binding.project: binding.repo_url for binding in remote_cfg.repos},
        provider=provider,
        ssh_ready_timeout_secs=remote_cfg.ssh.ssh_ready_timeout_secs,
        credential_providers=providers_for_clis(required_clis),
        agent_user=_AGENT_USER,
    )

    return env, state_store
