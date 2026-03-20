"""tanren run - standalone execution lifecycle commands.

Reference docs:
- docs/interfaces.md
- docs/getting-started/bootstrap.md
"""

from __future__ import annotations

import asyncio
import time
import uuid
from datetime import UTC, datetime
from pathlib import Path
from typing import TYPE_CHECKING, Annotated, Literal, cast

if TYPE_CHECKING:
    import asyncpg

    from tanren_core.adapters.ssh_environment import SSHExecutionEnvironment

import typer
import yaml
from pydantic import BaseModel, ConfigDict, Field, ValidationError

from tanren_core.adapters.null_emitter import NullEventEmitter
from tanren_core.adapters.postgres_pool import is_postgres_url
from tanren_core.adapters.remote_types import VMHandle, WorkspacePath
from tanren_core.adapters.ssh import SSHConfig, SSHConnection
from tanren_core.adapters.types import EnvironmentHandle, RemoteEnvironmentRuntime
from tanren_core.config import Config
from tanren_core.env.environment_schema import (
    EnvironmentProfile,
    EnvironmentProfileType,
    parse_environment_profiles,
)
from tanren_core.manager import build_tail_output
from tanren_core.roles import AgentTool, AuthMode, RoleName
from tanren_core.roles_config import load_roles_config
from tanren_core.schemas import Cli, Dispatch, Outcome, Phase

run_app = typer.Typer(help="Run provision/execute/teardown lifecycle without coordinator.")


class PersistedSSHDefaults(BaseModel):
    """Persisted SSH defaults for handle reconstruction."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    user: str = Field(...)
    key_path: str = Field(...)
    port: int = Field(...)
    connect_timeout: int = Field(...)
    host_key_policy: Literal["auto_add", "warn", "reject"] = Field(default="auto_add")


class PersistedRunHandle(BaseModel):
    """Serialized remote environment handle used by tanren run commands."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    env_id: str = Field(...)
    vm_id: str = Field(...)
    project: str = Field(...)
    branch: str = Field(...)
    workflow_id: str = Field(...)
    environment_profile: str = Field(...)
    local_worktree_path: str = Field(...)
    workspace_path: str = Field(...)
    teardown_commands: tuple[str, ...] = Field(default_factory=tuple)
    provisioned_at_utc: str = Field(...)
    vm_handle: VMHandle = Field(...)
    ssh_defaults: PersistedSSHDefaults = Field(...)


def _load_config() -> Config:
    try:
        return Config.from_env()
    except Exception as exc:
        typer.echo(f"Failed to load config from WM_* environment: {exc}", err=True)
        raise typer.Exit(code=1) from exc


def _require_remote_config(config: Config) -> None:
    if not config.remote_config_path:
        typer.echo("WM_REMOTE_CONFIG is required for tanren run commands.", err=True)
        raise typer.Exit(code=1)


async def _build_remote_execution_env(
    config: Config,
) -> tuple[SSHExecutionEnvironment, asyncpg.Pool | None]:
    """Build SSH execution environment, returning (env, pg_pool_or_none).

    Returns:
        Tuple of (SSHExecutionEnvironment, asyncpg.Pool | None).
    """
    from tanren_core.builder import (  # noqa: PLC0415 — avoid circular import
        build_ssh_execution_environment,
    )

    pool = None
    if config.events_db and is_postgres_url(config.events_db):
        from tanren_core.adapters.postgres_pool import (  # noqa: PLC0415 — optional dep
            create_postgres_pool,
        )

        pool = await create_postgres_pool(config.events_db)

    env, _store = build_ssh_execution_environment(config, NullEventEmitter(), pool=pool)
    return env, pool


def _handle_dir(config: Config) -> Path:
    path = Path(config.data_dir) / "run-handles"
    path.mkdir(parents=True, exist_ok=True)
    return path


def _handle_path(config: Config, env_id: str) -> Path:
    return _handle_dir(config) / f"{env_id}.json"


def _save_handle(config: Config, persisted: PersistedRunHandle) -> Path:
    path = _handle_path(config, persisted.env_id)
    path.write_text(persisted.model_dump_json(indent=2))
    return path


def _is_managed_handle(config: Config, handle_path: Path) -> bool:
    """Return True if handle_path lives inside the managed run-handles directory."""
    try:
        handle_path.resolve().relative_to(_handle_dir(config).resolve())
        return True
    except ValueError:
        return False


def _is_explicit_path(identifier: str) -> bool:
    """True if identifier contains a path separator, indicating an explicit file path.

    Returns:
        True if the identifier looks like a file path.
    """
    return "/" in identifier or "\\" in identifier


def _load_handle(config: Config, identifier: str) -> tuple[PersistedRunHandle, Path]:
    def _parse(path: Path) -> PersistedRunHandle:
        try:
            parsed = PersistedRunHandle.model_validate_json(path.read_text())
        except ValidationError as exc:
            typer.echo(f"Invalid run handle schema in {path}: {exc}", err=True)
            typer.echo(
                "Run handle schema has changed. Re-provision with `tanren run provision`.",
                err=True,
            )
            raise typer.Exit(code=1) from exc
        try:
            _parse_provisioned_at_utc(parsed.provisioned_at_utc)
        except ValueError as exc:
            typer.echo(f"Invalid run handle timestamp in {path}: {exc}", err=True)
            raise typer.Exit(code=1) from exc
        return parsed

    # 1. Registry lookup by env_id
    direct = _handle_path(config, identifier)
    if direct.exists():
        return _parse(direct), direct

    # 2. Registry lookup by vm_id
    for candidate in _handle_dir(config).glob("*.json"):
        loaded = _parse(candidate)
        if loaded.vm_id == identifier:
            return loaded, candidate

    # 3. Explicit file path (only for path-like identifiers)
    if _is_explicit_path(identifier):
        file_path = Path(identifier)
        if file_path.is_file():
            return _parse(file_path), file_path

    typer.echo(f"No run handle found for identifier: {identifier}", err=True)
    raise typer.Exit(code=1)


def _profile_for_runtime(name: str) -> EnvironmentProfile:
    return EnvironmentProfile(name=name, type=EnvironmentProfileType.REMOTE)


def _resolve_profile(config: Config, project: str, environment_profile: str) -> EnvironmentProfile:
    tanren_yml = Path(config.github_dir) / project / "tanren.yml"
    if tanren_yml.exists():
        loaded = yaml.safe_load(tanren_yml.read_text()) or {}
        data = loaded if isinstance(loaded, dict) else {}
        profiles = parse_environment_profiles(data)
    else:
        profiles = parse_environment_profiles({})
    profile = profiles.get(environment_profile)
    if profile is None:
        available = sorted(profiles.keys())
        raise ValueError(
            f"Environment profile '{environment_profile}' not found in tanren.yml. "
            f"Available: {available}"
        )
    return profile


def _resolve_gate_cmd(
    *,
    config: Config,
    project: str,
    environment_profile: str,
    phase: Phase,
    gate_cmd: str | None,
) -> str | None:
    if phase != Phase.GATE:
        return gate_cmd

    resolved = gate_cmd
    if resolved is None:
        resolved = _resolve_profile(config, project, environment_profile).gate_cmd

    normalized = resolved.strip() if resolved is not None else ""
    if not normalized:
        typer.echo(
            "Gate phase requires a non-empty gate command. "
            "Provide --gate-cmd or configure environment.<profile>.gate_cmd in tanren.yml.",
            err=True,
        )
        raise typer.Exit(code=1)
    return normalized


def _reconstruct_handle(persisted: PersistedRunHandle) -> EnvironmentHandle:
    ssh_cfg = SSHConfig(
        host=persisted.vm_handle.host,
        user=persisted.ssh_defaults.user,
        key_path=persisted.ssh_defaults.key_path,
        port=persisted.ssh_defaults.port,
        connect_timeout=persisted.ssh_defaults.connect_timeout,
        host_key_policy=persisted.ssh_defaults.host_key_policy,
    )
    conn = SSHConnection(ssh_cfg)

    return EnvironmentHandle(
        env_id=persisted.env_id,
        worktree_path=Path(persisted.local_worktree_path),
        branch=persisted.branch,
        project=persisted.project,
        runtime=RemoteEnvironmentRuntime(
            vm_handle=persisted.vm_handle,
            connection=conn,
            workspace_path=WorkspacePath(
                path=persisted.workspace_path,
                project=persisted.project,
                branch=persisted.branch,
            ),
            profile=_profile_for_runtime(persisted.environment_profile),
            teardown_commands=persisted.teardown_commands,
            provision_start=_provision_start_monotonic(persisted.provisioned_at_utc),
            workflow_id=persisted.workflow_id,
        ),
    )


def _now_utc_iso() -> str:
    return datetime.now(UTC).isoformat()


def _parse_provisioned_at_utc(value: str) -> datetime:
    try:
        parsed = datetime.fromisoformat(value)
    except ValueError as exc:
        raise ValueError(f"Invalid provisioned_at_utc timestamp: {value}") from exc
    if parsed.tzinfo is None:
        raise ValueError(f"provisioned_at_utc must include timezone offset: {value}")
    return parsed.astimezone(UTC)


def _elapsed_seconds(provisioned_at_utc: str) -> float:
    provisioned_at = _parse_provisioned_at_utc(provisioned_at_utc)
    return max(0.0, (datetime.now(UTC) - provisioned_at).total_seconds())


def _provision_start_monotonic(provisioned_at_utc: str) -> float:
    # Reconstruct a process-local monotonic baseline from persisted wall-clock start.
    return time.monotonic() - _elapsed_seconds(provisioned_at_utc)


def _role_for_phase(phase: Phase) -> RoleName:
    if phase in (Phase.AUDIT_TASK, Phase.AUDIT_SPEC):
        return RoleName.AUDIT
    if phase == Phase.RUN_DEMO:
        return RoleName.FEEDBACK
    if phase == Phase.DO_TASK:
        return RoleName.IMPLEMENTATION
    if phase == Phase.INVESTIGATE:
        return RoleName.CONVERSATION
    return RoleName.DEFAULT


def _resolve_agent_tool(config: Config, phase: Phase) -> AgentTool:
    if phase == Phase.GATE:
        return AgentTool(cli=Cli.BASH, auth=AuthMode.API_KEY)
    return load_roles_config(config.roles_config_path).resolve(_role_for_phase(phase))


def _build_dispatch(
    *,
    project: str,
    phase: Phase,
    spec_path: str,
    branch: str,
    environment_profile: str,
    workflow_id: str,
    timeout: int,
    context: str | None,
    gate_cmd: str | None,
    tool: AgentTool,
) -> Dispatch:
    return Dispatch(
        workflow_id=workflow_id,
        phase=phase,
        project=project,
        spec_folder=spec_path,
        branch=branch,
        cli=tool.cli,
        auth=tool.auth,
        model=tool.model,
        gate_cmd=gate_cmd,
        context=context,
        timeout=timeout,
        environment_profile=environment_profile,
    )


@run_app.command("provision")
def run_provision(
    project: Annotated[str, typer.Option(..., "--project")],
    branch: Annotated[str, typer.Option(..., "--branch")],
    environment_profile: Annotated[str, typer.Option("--environment-profile")] = "default",
) -> None:
    """Provision a remote execution environment and persist its handle."""

    async def _run() -> None:
        config = _load_config()
        _require_remote_config(config)
        env, pg_pool = await _build_remote_execution_env(config)
        try:
            workflow_id = f"run-{project}-{uuid.uuid4().hex[:10]}"
            tool = _resolve_agent_tool(config, Phase.DO_TASK)
            dispatch = Dispatch(
                workflow_id=workflow_id,
                phase=Phase.DO_TASK,
                project=project,
                spec_folder=".",
                branch=branch,
                cli=tool.cli,
                auth=tool.auth,
                model=None,
                gate_cmd=None,
                context=None,
                timeout=1800,
                environment_profile=environment_profile,
            )
            handle = await env.provision(dispatch, config)
            runtime = cast("RemoteEnvironmentRuntime", handle.runtime)
            vm = runtime.vm_handle

            # Close the SSH connection from provision (not needed after handle save)
            conn = cast("SSHConnection", runtime.connection)
            await conn.close()

            persisted = PersistedRunHandle(
                env_id=handle.env_id,
                vm_id=vm.vm_id,
                project=project,
                branch=branch,
                workflow_id=workflow_id,
                environment_profile=environment_profile,
                local_worktree_path=str(Path(config.github_dir) / project),
                workspace_path=runtime.workspace_path.path,
                teardown_commands=runtime.teardown_commands,
                provisioned_at_utc=_now_utc_iso(),
                vm_handle=runtime.vm_handle,
                ssh_defaults=PersistedSSHDefaults(
                    user=env.ssh_defaults.user,
                    key_path=env.ssh_defaults.key_path,
                    port=env.ssh_defaults.port,
                    connect_timeout=env.ssh_defaults.connect_timeout,
                    host_key_policy=env.ssh_defaults.host_key_policy,
                ),
            )
            path = _save_handle(config, persisted)

            typer.echo(f"env_id: {persisted.env_id}")
            typer.echo(f"vm_id: {persisted.vm_id}")
            typer.echo(f"host: {vm.host}")
            typer.echo(f"ssh: ssh {persisted.ssh_defaults.user}@{vm.host}")
            typer.echo(
                "vscode: "
                f"code --folder-uri vscode-remote://ssh-remote+"
                f"{persisted.ssh_defaults.user}@{vm.host}"
                f"{runtime.workspace_path.path}"
            )
            typer.echo(f"handle_file: {path}")
        finally:
            await env.close()
            if pg_pool is not None:
                await pg_pool.close()

    asyncio.run(_run())


@run_app.command("execute")
def run_execute(
    handle: Annotated[str, typer.Option(..., "--handle")],
    project: Annotated[str, typer.Option(..., "--project")],
    spec_path: Annotated[str, typer.Option(..., "--spec-path")],
    phase: Annotated[Phase, typer.Option(..., "--phase")],
    timeout: Annotated[int, typer.Option("--timeout")] = 1800,
    context: Annotated[str | None, typer.Option("--context")] = None,
    gate_cmd: Annotated[str | None, typer.Option("--gate-cmd")] = None,
) -> None:
    """Execute one phase against a previously provisioned environment."""

    async def _run() -> None:
        config = _load_config()
        _require_remote_config(config)
        env, pg_pool = await _build_remote_execution_env(config)
        try:
            persisted, _ = _load_handle(config, handle)
            if persisted.project != project:
                typer.echo(
                    f"Handle project mismatch: expected {persisted.project}, got {project}",
                    err=True,
                )
                raise typer.Exit(code=1)

            tool = _resolve_agent_tool(config, phase)
            resolved_gate_cmd = _resolve_gate_cmd(
                config=config,
                project=project,
                environment_profile=persisted.environment_profile,
                phase=phase,
                gate_cmd=gate_cmd,
            )
            dispatch = _build_dispatch(
                project=project,
                phase=phase,
                spec_path=spec_path,
                branch=persisted.branch,
                environment_profile=persisted.environment_profile,
                workflow_id=persisted.workflow_id,
                timeout=timeout,
                context=context,
                gate_cmd=resolved_gate_cmd,
                tool=tool,
            )
            env_handle = _reconstruct_handle(persisted)
            runtime = cast("RemoteEnvironmentRuntime", env_handle.runtime)
            conn = cast("SSHConnection", runtime.connection)

            try:
                result = await env.execute(env_handle, dispatch, config)
            finally:
                await conn.close()

            typer.echo(f"outcome: {result.outcome.value}")
            typer.echo(f"signal: {result.signal}")
            typer.echo(f"exit_code: {result.exit_code}")
            typer.echo(f"duration_secs: {result.duration_secs}")
            if result.token_usage:
                typer.echo(f"token_cost: ${result.token_usage.get('total_cost', 0):.4f}")
                typer.echo(f"token_total: {result.token_usage.get('total_tokens', 0)}")
            tail = build_tail_output(result.stdout)
            if tail:
                typer.echo("stdout_tail:")
                typer.echo(tail)
            if result.stderr:
                stderr_tail = build_tail_output(result.stderr)
                if stderr_tail:
                    typer.echo("stderr_tail:")
                    typer.echo(stderr_tail)
        finally:
            await env.close()
            if pg_pool is not None:
                await pg_pool.close()

    asyncio.run(_run())


@run_app.command("teardown")
def run_teardown(
    handle: Annotated[str, typer.Option(..., "--handle")],
) -> None:
    """Teardown a previously provisioned environment."""

    async def _run() -> None:
        config = _load_config()
        _require_remote_config(config)
        env, pg_pool = await _build_remote_execution_env(config)
        try:
            persisted, handle_path = _load_handle(config, handle)

            env_handle = _reconstruct_handle(persisted)
            await env.teardown(env_handle)

            duration = _elapsed_seconds(persisted.provisioned_at_utc)
            estimated_cost: float | None = None
            vm_hourly_cost = persisted.vm_handle.hourly_cost
            if vm_hourly_cost is not None:
                estimated_cost = vm_hourly_cost * (duration / 3600.0)

            typer.echo(f"released_vm_id: {persisted.vm_id}")
            if _is_managed_handle(config, handle_path):
                handle_path.unlink(missing_ok=True)
                typer.echo(f"removed_handle: {handle_path}")
            else:
                typer.echo(f"handle_retained: {handle_path} (external file)")
            if estimated_cost is not None:
                typer.echo(f"estimated_cost: {estimated_cost:.4f}")
        finally:
            await env.close()
            if pg_pool is not None:
                await pg_pool.close()

    asyncio.run(_run())


@run_app.command("full")
def run_full(
    project: Annotated[str, typer.Option(..., "--project")],
    branch: Annotated[str, typer.Option(..., "--branch")],
    spec_path: Annotated[str, typer.Option(..., "--spec-path")],
    phase: Annotated[Phase, typer.Option(..., "--phase")],
    environment_profile: Annotated[str, typer.Option("--environment-profile")] = "default",
    timeout: Annotated[int, typer.Option("--timeout")] = 1800,
    context: Annotated[str | None, typer.Option("--context")] = None,
    gate_cmd: Annotated[str | None, typer.Option("--gate-cmd")] = None,
) -> None:
    """Run provision -> execute -> teardown, always tearing down in finally."""

    async def _run() -> None:
        config = _load_config()
        _require_remote_config(config)
        env, pg_pool = await _build_remote_execution_env(config)
        try:
            workflow_id = f"run-{project}-{uuid.uuid4().hex[:10]}"
            provision_tool = _resolve_agent_tool(config, Phase.DO_TASK)
            provision_dispatch = Dispatch(
                workflow_id=workflow_id,
                phase=Phase.DO_TASK,
                project=project,
                spec_folder=spec_path,
                branch=branch,
                cli=provision_tool.cli,
                auth=provision_tool.auth,
                model=None,
                gate_cmd=None,
                context=None,
                timeout=timeout,
                environment_profile=environment_profile,
            )
            handle = await env.provision(provision_dispatch, config)
            runtime = cast("RemoteEnvironmentRuntime", handle.runtime)
            vm = runtime.vm_handle

            # Close the SSH connection from provision (not needed after handle save)
            conn = cast("SSHConnection", runtime.connection)
            await conn.close()

            persisted = PersistedRunHandle(
                env_id=handle.env_id,
                vm_id=vm.vm_id,
                project=project,
                branch=branch,
                workflow_id=workflow_id,
                environment_profile=environment_profile,
                local_worktree_path=str(Path(config.github_dir) / project),
                workspace_path=runtime.workspace_path.path,
                teardown_commands=runtime.teardown_commands,
                provisioned_at_utc=_now_utc_iso(),
                vm_handle=runtime.vm_handle,
                ssh_defaults=PersistedSSHDefaults(
                    user=env.ssh_defaults.user,
                    key_path=env.ssh_defaults.key_path,
                    port=env.ssh_defaults.port,
                    connect_timeout=env.ssh_defaults.connect_timeout,
                    host_key_policy=env.ssh_defaults.host_key_policy,
                ),
            )
            handle_path: Path | None = None
            execute_failed = False
            try:
                handle_path = _save_handle(config, persisted)
                tool = _resolve_agent_tool(config, phase)
                resolved_gate_cmd = _resolve_gate_cmd(
                    config=config,
                    project=project,
                    environment_profile=environment_profile,
                    phase=phase,
                    gate_cmd=gate_cmd,
                )
                dispatch = _build_dispatch(
                    project=project,
                    phase=phase,
                    spec_path=spec_path,
                    branch=branch,
                    environment_profile=environment_profile,
                    workflow_id=workflow_id,
                    timeout=timeout,
                    context=context,
                    gate_cmd=resolved_gate_cmd,
                    tool=tool,
                )
                exec_handle = _reconstruct_handle(persisted)
                exec_runtime = cast("RemoteEnvironmentRuntime", exec_handle.runtime)
                exec_conn = cast("SSHConnection", exec_runtime.connection)
                try:
                    result = await env.execute(exec_handle, dispatch, config)
                finally:
                    await exec_conn.close()

                typer.echo(f"outcome: {result.outcome.value}")
                typer.echo(f"signal: {result.signal}")
                typer.echo(f"exit_code: {result.exit_code}")
                typer.echo(f"duration_secs: {result.duration_secs}")
                if result.token_usage:
                    typer.echo(f"token_cost: ${result.token_usage.get('total_cost', 0):.4f}")
                    typer.echo(f"token_total: {result.token_usage.get('total_tokens', 0)}")
                if result.stderr:
                    stderr_tail = build_tail_output(result.stderr)
                    if stderr_tail:
                        typer.echo("stderr_tail:")
                        typer.echo(stderr_tail)
                if result.outcome != Outcome.SUCCESS:
                    execute_failed = True
            finally:
                teardown_handle = _reconstruct_handle(persisted)
                await env.teardown(teardown_handle)
                if handle_path is not None and _is_managed_handle(config, handle_path):
                    handle_path.unlink(missing_ok=True)

            typer.echo(f"provisioned_vm_id: {persisted.vm_id}")
            typer.echo("teardown: completed")
            if execute_failed:
                raise typer.Exit(code=1)
        finally:
            await env.close()
            if pg_pool is not None:
                await pg_pool.close()

    asyncio.run(_run())


# Backward-compatible import name.
run = run_app
