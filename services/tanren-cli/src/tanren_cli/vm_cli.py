"""tanren vm - manage remote VM assignments."""

from __future__ import annotations

import asyncio
from pathlib import Path

import typer
import yaml
from dotenv import dotenv_values

from tanren_core.adapters.manual_vm import ManualProvisionerSettings
from tanren_core.adapters.sqlite_vm_state import SqliteVMStateStore
from tanren_core.adapters.ssh import SSHConfig, SSHConnection
from tanren_core.adapters.ubuntu_bootstrap import UbuntuBootstrapper
from tanren_core.config import Config
from tanren_core.env.environment_schema import EnvironmentProfile, parse_environment_profiles
from tanren_core.remote_config import ProvisionerType, RemoteSSHConfig, load_remote_config
from tanren_core.roles_config import load_roles_config
from tanren_core.secrets import SecretConfig, SecretLoader

vm_app = typer.Typer(help="Manage remote VM assignments.")


def _load_config() -> Config:
    """Load worker-manager Config, exiting on failure.

    Returns:
        Loaded Config instance.

    Raises:
        Exit: If the configuration cannot be loaded.
    """
    try:
        return Config.from_env()
    except Exception as exc:
        typer.echo(f"Failed to load config: {exc}", err=True)
        raise typer.Exit(code=1) from exc


def _get_state_store(config: Config) -> SqliteVMStateStore:
    """Create a VMStateStore from Config.data_dir.

    Returns:
        SqliteVMStateStore backed by the config data directory.
    """
    db_path = f"{config.data_dir}/vm-state.db"
    return SqliteVMStateStore(db_path)


@vm_app.command("list")
def vm_list() -> None:
    """Show active VM assignments."""

    async def _run() -> None:
        config = _load_config()
        store = _get_state_store(config)
        try:
            assignments = await store.get_active_assignments()
            if not assignments:
                typer.echo("No active VM assignments.")
                return
            typer.echo(
                f"{'VM ID':<20} {'Host':<20} {'Workflow':<30} {'Project':<15} {'Assigned At'}"
            )
            typer.echo("-" * 100)
            for assignment in assignments:
                typer.echo(
                    f"{assignment.vm_id:<20} {assignment.host:<20} "
                    f"{assignment.workflow_id:<30} {assignment.project:<15} "
                    f"{assignment.assigned_at}"
                )
        finally:
            await store.close()

    asyncio.run(_run())


@vm_app.command("release")
def vm_release(vm_id: str = typer.Argument(...)) -> None:
    """Manually release a stuck VM assignment."""

    async def _run() -> None:
        config = _load_config()
        store = _get_state_store(config)
        try:
            assignment = await store.get_assignment(vm_id)
            if assignment is None:
                typer.echo(f"No active assignment found for VM: {vm_id}", err=True)
                raise typer.Exit(code=1)
            await store.record_release(vm_id)
            typer.echo(f"Released VM {vm_id} (was assigned to {assignment.workflow_id})")
        finally:
            await store.close()

    asyncio.run(_run())


@vm_app.command("recover")
def vm_recover() -> None:
    """Run startup recovery: verify connectivity, release unreachable VMs."""

    async def _run() -> None:
        config = _load_config()
        store = _get_state_store(config)
        try:
            assignments = await store.get_active_assignments()
            if not assignments:
                typer.echo("No active assignments to recover.")
                return

            typer.echo(f"Checking {len(assignments)} active assignment(s)...")

            ssh_defaults: RemoteSSHConfig | None = None
            if config.remote_config_path:
                ssh_defaults = load_remote_config(config.remote_config_path).ssh

            for assignment in assignments:
                if ssh_defaults is not None:
                    ssh_config = SSHConfig(
                        host=assignment.host,
                        user=ssh_defaults.user,
                        key_path=ssh_defaults.key_path,
                        port=ssh_defaults.port,
                        connect_timeout=ssh_defaults.connect_timeout,
                        host_key_policy=ssh_defaults.host_key_policy,
                    )
                else:
                    ssh_config = SSHConfig(host=assignment.host)
                conn = SSHConnection(ssh_config)
                try:
                    reachable = await conn.check_connection()
                    if reachable:
                        typer.echo(f"  {assignment.vm_id} ({assignment.host}): reachable")
                    else:
                        await store.record_release(assignment.vm_id)
                        typer.echo(
                            f"  {assignment.vm_id} ({assignment.host}): UNREACHABLE - released"
                        )
                except Exception:
                    await store.record_release(assignment.vm_id)
                    typer.echo(f"  {assignment.vm_id} ({assignment.host}): ERROR - released")
                finally:
                    await conn.close()
        finally:
            await store.close()

    asyncio.run(_run())


@vm_app.command("dry-run")
def vm_dry_run(
    project: str = typer.Option(..., "--project"),
    environment_profile: str = typer.Option("default", "--environment-profile"),
) -> None:
    """Show what remote provision would do without creating resources.

    Raises:
        Exit: If WM_REMOTE_CONFIG is not set or the provisioner type
            is unsupported.
    """
    config = _load_config()
    if not config.remote_config_path:
        typer.echo("WM_REMOTE_CONFIG is required for vm dry-run.", err=True)
        raise typer.Exit(code=1)

    remote_cfg = load_remote_config(config.remote_config_path)

    tanren_yml = Path(config.github_dir) / project / "tanren.yml"
    if tanren_yml.exists():
        raw = yaml.safe_load(tanren_yml.read_text()) or {}
        data = raw if isinstance(raw, dict) else {}
        profiles = parse_environment_profiles(data)
    else:
        profiles = parse_environment_profiles({})
    profile = profiles.get(environment_profile, EnvironmentProfile(name=environment_profile))

    typer.echo(f"project: {project}")
    typer.echo(f"profile: {profile.name}")
    typer.echo(f"provisioner: {remote_cfg.provisioner.type.value}")

    if remote_cfg.provisioner.type == ProvisionerType.HETZNER:
        from tanren_core.adapters.hetzner_vm import HetznerProvisionerSettings  # noqa: PLC0415

        settings = HetznerProvisionerSettings.from_settings(remote_cfg.provisioner.settings)
        resolved_server_type = profile.server_type or settings.default_server_type
        source = (
            "profile.server_type"
            if profile.server_type
            else "provisioner.settings.default_server_type"
        )
        typer.echo(f"server_type: {resolved_server_type} ({source})")
        typer.echo(f"location: {settings.location}")
        typer.echo(f"image: {settings.image}")
        typer.echo(f"ssh_key_name: {settings.ssh_key_name}")
    elif remote_cfg.provisioner.type == ProvisionerType.MANUAL:
        settings = ManualProvisionerSettings.from_settings(remote_cfg.provisioner.settings)
        typer.echo(f"manual_vm_pool_size: {len(settings.vms)}")
    else:
        typer.echo(f"unsupported provisioner type: {remote_cfg.provisioner.type}", err=True)
        raise typer.Exit(code=1)

    # Load roles to determine required CLIs
    roles = load_roles_config(config.roles_config_path)
    required_clis = roles.required_clis()

    typer.echo("bootstrap_steps:")
    bootstrapper = UbuntuBootstrapper(required_clis=required_clis)
    bootstrap_plan = bootstrapper.plan()
    typer.echo(f"  apt: {' '.join(bootstrap_plan.apt_packages)}")
    for step in bootstrap_plan.install_steps:
        typer.echo(f"  install: {step.name}")
    if remote_cfg.bootstrap.extra_script:
        typer.echo(f"  extra_script: {remote_cfg.bootstrap.extra_script}")

    repo_url = remote_cfg.repo_url_for(project)
    if repo_url:
        typer.echo(f"repo_clone: {repo_url}")
    else:
        typer.echo("repo_clone: <missing repo mapping>")

    typer.echo("setup_commands:")
    if profile.setup:
        for cmd in profile.setup:
            typer.echo(f"  - {cmd}")
    else:
        typer.echo("  - <none>")

    project_env_file = Path(config.github_dir) / project / ".env"
    project_secret_keys = []
    if project_env_file.exists():
        project_secret_keys = sorted(dotenv_values(project_env_file).keys())

    secret_config = SecretConfig(
        developer_secrets_path=(
            remote_cfg.secrets.developer_secrets_path or SecretConfig().developer_secrets_path
        ),
    )
    loader = SecretLoader(secret_config, required_clis=required_clis)
    developer_secret_keys = sorted(loader.load_developer().keys())
    infrastructure_secret_keys = sorted(secret_config.infrastructure_env_vars)

    typer.echo("secret_keys:")
    developer_display = ", ".join(developer_secret_keys) if developer_secret_keys else "<none>"
    project_display = ", ".join(project_secret_keys) if project_secret_keys else "<none>"
    typer.echo(f"  developer: {developer_display}")
    typer.echo(f"  project_env: {project_display}")
    typer.echo(
        "  infrastructure: "
        f"{', '.join(infrastructure_secret_keys) if infrastructure_secret_keys else '<none>'}"
    )


# Backward-compatible name imported by top-level CLI module.
vm = vm_app
