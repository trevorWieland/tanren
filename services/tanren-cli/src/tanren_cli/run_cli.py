"""tanren run - standalone execution lifecycle commands.

Reference docs:
- docs/interfaces.md
- docs/getting-started/bootstrap.md
"""

from __future__ import annotations

import asyncio
import shutil
import tempfile
import uuid
from datetime import UTC, datetime
from pathlib import Path
from typing import TYPE_CHECKING, Annotated

import typer
import yaml

from tanren_core.builder import build_execution_environment
from tanren_core.env.environment_schema import (
    DispatchGitConfig,
    DispatchProvisionerConfig,
    EnvironmentProfile,
    EnvironmentProfileType,
    RemoteExecutionConfig,
    SSHDefaults,
    parse_environment_profiles,
)
from tanren_core.env.gates import resolve_gate_cmd
from tanren_core.roles import AgentTool, AuthMode, RoleName
from tanren_core.roles_config import load_roles_config
from tanren_core.schemas import Cli, Dispatch, Phase
from tanren_core.store.enums import DispatchMode, StepStatus, StepType, cli_to_lane
from tanren_core.store.events import DispatchCreated
from tanren_core.store.factory import create_sqlite_store
from tanren_core.store.payloads import (
    ExecuteResult,
    ExecuteStepPayload,
    ProvisionResult,
    ProvisionStepPayload,
    TeardownStepPayload,
)
from tanren_core.worker import Worker
from tanren_core.worker_config import WorkerConfig

if TYPE_CHECKING:
    from tanren_core.store.sqlite import SqliteStore

run_app = typer.Typer(help="Run provision/execute/teardown lifecycle without coordinator.")


def _load_config() -> WorkerConfig:
    try:
        return WorkerConfig.from_env()
    except Exception as exc:
        typer.echo(f"Failed to load config from WM_* environment: {exc}", err=True)
        raise typer.Exit(code=1) from exc


def _resolve_profile(
    config: WorkerConfig, project: str, environment_profile: str
) -> EnvironmentProfile:
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

    # For remote profiles, populate remote_config from remote.yml + roles.yml
    if profile.type == EnvironmentProfileType.REMOTE and profile.remote_config is None:
        remote_cfg = _resolve_remote_config(config, project)
        # Rebuild profile with remote_config (frozen model requires reconstruction)
        profile = profile.model_copy(update={"remote_config": remote_cfg})

    return profile


def _resolve_remote_config(config: WorkerConfig, project: str) -> RemoteExecutionConfig:
    """Read remote.yml + roles.yml and build dispatch-carried RemoteExecutionConfig.

    Returns:
        Fully resolved RemoteExecutionConfig for the dispatch payload.

    Raises:
        ValueError: If WM_REMOTE_CONFIG is not set.
    """
    from tanren_core.remote_config import load_remote_config  # noqa: PLC0415

    if not config.remote_config_path:
        raise ValueError("WM_REMOTE_CONFIG is required for remote profiles")

    remote = load_remote_config(config.remote_config_path)

    # Read bootstrap extra_script content (if configured)
    extra_script = None
    if remote.bootstrap.extra_script:
        script_path = Path(remote.bootstrap.extra_script).expanduser()
        if not script_path.is_absolute():
            config_dir = Path(config.remote_config_path).resolve().parent
            script_path = config_dir / script_path
        if script_path.exists():
            extra_script = script_path.read_text()

    # Resolve required CLIs from roles.yml
    required_clis: tuple[str, ...] = ()
    if config.roles_config_path:
        roles = load_roles_config(config.roles_config_path)
        required_clis = tuple(str(c) for c in roles.required_clis())

    # Look up repo URL for this project
    repo_url = remote.repo_url_for(project) or ""

    return RemoteExecutionConfig(
        ssh=SSHDefaults(
            user=remote.ssh.user,
            key_path=remote.ssh.key_path,
            port=remote.ssh.port,
            connect_timeout=remote.ssh.connect_timeout,
            host_key_policy=remote.ssh.host_key_policy,
            ssh_ready_timeout_secs=remote.ssh.ssh_ready_timeout_secs,
        ),
        git=DispatchGitConfig(
            auth_method=str(remote.git.auth),
            token_env=remote.git.token_env,
        ),
        provisioner=DispatchProvisionerConfig(
            type=str(remote.provisioner.type),
            settings=dict(remote.provisioner.settings),
        ),
        repo_url=repo_url,
        required_clis=required_clis,
        bootstrap_extra_script=extra_script,
    )


def _resolve_project_env(config: WorkerConfig, project: str) -> dict[str, str]:
    """Read project .env file for dispatch payload.

    Returns:
        Dict of env var key-value pairs from the project .env file.
    """
    from dotenv import dotenv_values  # noqa: PLC0415

    env_file = Path(config.github_dir) / project / ".env"
    if not env_file.exists():
        return {}
    values = dotenv_values(env_file)
    return {k: v for k, v in values.items() if v is not None}


async def _resolve_cloud_secrets(config: WorkerConfig, project: str) -> dict[str, str]:
    """Fetch cloud secrets for vars with ``source: "secret:X"`` in tanren.yml.

    Returns:
        Dict of secret name to value for vars with cloud secret sources.
    """
    tanren_yml = Path(config.github_dir) / project / "tanren.yml"
    if not tanren_yml.exists():
        return {}

    from tanren_core.env.schema import TanrenConfig  # noqa: PLC0415

    data = yaml.safe_load(tanren_yml.read_text()) or {}
    if not isinstance(data, dict):
        return {}
    try:
        tc = TanrenConfig.model_validate(data)
    except Exception:
        return {}

    if tc.env is None:
        return {}

    has_sources = any(v.source for v in (*tc.env.required, *tc.env.optional))
    if not has_sources:
        return {}

    from tanren_core.env.secret_provider_factory import create_secret_provider  # noqa: PLC0415

    provider = create_secret_provider(tc.secrets)
    result: dict[str, str] = {}
    for var in (*tc.env.required, *tc.env.optional):
        if var.source and var.source.startswith("secret:"):
            secret_name = var.source[len("secret:") :]
            value = await provider.get_secret(secret_name)
            if value is not None:
                result[var.key] = value
    return result


def _resolve_gate_cmd_for_phase(
    *,
    config: WorkerConfig,
    project: str,
    environment_profile: str,
    phase: Phase,
    gate_cmd: str | None,
) -> str | None:
    if phase != Phase.GATE:
        return gate_cmd

    resolved = gate_cmd
    if resolved is None:
        profile = _resolve_profile(config, project, environment_profile)
        resolved = resolve_gate_cmd(profile, phase)

    normalized = resolved.strip() if resolved is not None else ""
    if not normalized:
        typer.echo(
            "Gate phase requires a non-empty gate command. "
            "Provide --gate-cmd or configure environment.<profile>.gate_cmd in tanren.yml.",
            err=True,
        )
        raise typer.Exit(code=1)
    return normalized


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


def _resolve_agent_tool(config: WorkerConfig, phase: Phase) -> AgentTool:
    if phase == Phase.GATE:
        return AgentTool(cli=Cli.BASH, auth=AuthMode.API_KEY)
    if not config.roles_config_path:
        raise ValueError("WM_ROLES_CONFIG_PATH is required for non-gate phases")
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
    resolved_profile: EnvironmentProfile,
    project_env: dict[str, str] | None = None,
    cloud_secrets: dict[str, str] | None = None,
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
        resolved_profile=resolved_profile,
        project_env=project_env or {},
        cloud_secrets=cloud_secrets or {},
    )


def _store_path(config: WorkerConfig) -> str:
    """Return the persistent store path for multi-step CLI workflows."""
    return str(Path(config.data_dir) / "run.db")


def _now() -> str:
    return datetime.now(UTC).isoformat()


async def _enqueue_dispatch(
    store: SqliteStore,
    dispatch: Dispatch,
    mode: DispatchMode,
) -> str:
    """Create dispatch projection, append event, enqueue provision step.

    Returns:
        The dispatch_id.
    """
    dispatch_id = dispatch.workflow_id
    lane = cli_to_lane(dispatch.cli)

    await store.create_dispatch_projection(
        dispatch_id=dispatch_id,
        mode=mode,
        lane=lane,
        preserve_on_failure=dispatch.preserve_on_failure,
        dispatch_json=dispatch.model_dump_json(),
    )
    await store.append(
        DispatchCreated(
            timestamp=_now(),
            workflow_id=dispatch_id,
            dispatch=dispatch,
            mode=mode,
            lane=lane,
        )
    )
    step_id = uuid.uuid4().hex
    payload = ProvisionStepPayload(dispatch=dispatch)
    await store.enqueue_step(
        step_id=step_id,
        dispatch_id=dispatch_id,
        step_type="provision",
        step_sequence=0,
        lane=None,
        payload_json=payload.model_dump_json(),
    )
    return dispatch_id


def _build_tail_output(text: str | None, max_lines: int = 30) -> str:
    """Return the last max_lines of text, or empty string."""
    if not text:
        return ""
    lines = text.strip().splitlines()
    return "\n".join(lines[-max_lines:])


def _make_env_factory(config: WorkerConfig, profile: EnvironmentProfile) -> tuple:
    """Create an env_factory closure that returns a pre-built environment.

    Returns:
        Tuple of (factory, execution_env, vm_store).
    """
    env, vm_store = build_execution_environment(config, profile)

    def factory(
        _cfg: WorkerConfig,
        _prof: EnvironmentProfile,
    ) -> tuple:
        return env, vm_store

    return factory, env, vm_store


@run_app.command("provision")
def run_provision(
    project: Annotated[str, typer.Option(..., "--project")],
    branch: Annotated[str, typer.Option(..., "--branch")],
    environment_profile: Annotated[str, typer.Option("--environment-profile")] = "default",
) -> None:
    """Provision an execution environment via embedded worker."""

    async def _run() -> None:
        config = _load_config()
        profile = _resolve_profile(config, project, environment_profile)

        db_path = _store_path(config)
        store = await create_sqlite_store(db_path)
        env_factory, execution_env, _vm_store = _make_env_factory(config, profile)

        try:
            workflow_id = (
                f"wf-{project}-cli-{uuid.uuid4().hex[:6]}-{int(datetime.now(UTC).timestamp())}"
            )
            tool = _resolve_agent_tool(config, Phase.DO_TASK)
            project_env = _resolve_project_env(config, project)
            cloud_secrets = await _resolve_cloud_secrets(config, project)
            dispatch = _build_dispatch(
                project=project,
                phase=Phase.DO_TASK,
                spec_path=".",
                branch=branch,
                environment_profile=environment_profile,
                workflow_id=workflow_id,
                timeout=1800,
                context=None,
                gate_cmd=None,
                tool=tool,
                resolved_profile=profile,
                project_env=project_env,
                cloud_secrets=cloud_secrets,
            )

            dispatch_id = await _enqueue_dispatch(store, dispatch, DispatchMode.MANUAL)

            worker = Worker(
                config=config,
                event_store=store,
                job_queue=store,
                state_store=store,
                env_factory=env_factory,
            )

            # Run until provision step completes
            await worker.run_until_step_complete(dispatch_id, StepType.PROVISION)

            # Read result
            steps = await store.get_steps_for_dispatch(dispatch_id)
            provision_step = next(s for s in steps if s.step_type == StepType.PROVISION)
            if provision_step.status == StepStatus.FAILED:
                typer.echo("Provision failed.", err=True)
                raise typer.Exit(code=1)

            if not provision_step.result_json:
                typer.echo("Provision step has no result.", err=True)
                raise typer.Exit(code=1)
            prov_result = ProvisionResult.model_validate_json(provision_step.result_json)
            handle = prov_result.handle

            typer.echo(f"dispatch_id: {dispatch_id}")
            typer.echo(f"env_id: {handle.env_id}")
            if handle.vm:
                typer.echo(f"vm_id: {handle.vm.vm_id}")
                typer.echo(f"host: {handle.vm.host}")
        finally:
            if hasattr(execution_env, "close"):
                await execution_env.close()
            await store.close()

    asyncio.run(_run())


@run_app.command("execute")
def run_execute(
    dispatch_id: Annotated[str, typer.Option(..., "--dispatch-id")],
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

        db_path = _store_path(config)
        store = await create_sqlite_store(db_path)

        try:
            # Read provision result from store
            steps = await store.get_steps_for_dispatch(dispatch_id)
            provision_step = next(
                (
                    s
                    for s in steps
                    if s.step_type == StepType.PROVISION and s.status == StepStatus.COMPLETED
                ),
                None,
            )
            if provision_step is None or provision_step.result_json is None:
                typer.echo(
                    f"No completed provision found for dispatch {dispatch_id}",
                    err=True,
                )
                raise typer.Exit(code=1)

            prov_result = ProvisionResult.model_validate_json(provision_step.result_json)
            view = await store.get_dispatch(dispatch_id)
            if view is None:
                typer.echo(f"Dispatch {dispatch_id} not found", err=True)
                raise typer.Exit(code=1)

            dispatch_data = view.dispatch
            profile = dispatch_data.resolved_profile
            env_factory, execution_env, _vm_store = _make_env_factory(config, profile)

            tool = _resolve_agent_tool(config, phase)
            resolved_gate_cmd = _resolve_gate_cmd_for_phase(
                config=config,
                project=project,
                environment_profile=dispatch_data.environment_profile,
                phase=phase,
                gate_cmd=gate_cmd,
            )

            exec_dispatch = _build_dispatch(
                project=project,
                phase=phase,
                spec_path=spec_path,
                branch=dispatch_data.branch,
                environment_profile=dispatch_data.environment_profile,
                workflow_id=dispatch_id,
                timeout=timeout,
                context=context,
                gate_cmd=resolved_gate_cmd,
                tool=tool,
                resolved_profile=profile,
            )

            lane = cli_to_lane(exec_dispatch.cli)
            existing_steps = await store.get_steps_for_dispatch(dispatch_id)
            max_seq = max((s.step_sequence for s in existing_steps), default=0)
            step_id = uuid.uuid4().hex
            payload = ExecuteStepPayload(dispatch=exec_dispatch, handle=prov_result.handle)
            await store.enqueue_step(
                step_id=step_id,
                dispatch_id=dispatch_id,
                step_type="execute",
                step_sequence=max_seq + 1,
                lane=str(lane),
                payload_json=payload.model_dump_json(),
            )

            worker = Worker(
                config=config,
                event_store=store,
                job_queue=store,
                state_store=store,
                env_factory=env_factory,
            )

            # Run until execute step completes
            await worker.run_until_step_complete(dispatch_id, StepType.EXECUTE)

            # Read result from the latest execute step
            steps = await store.get_steps_for_dispatch(dispatch_id)
            exec_steps = [s for s in steps if s.step_type == StepType.EXECUTE]
            exec_step = exec_steps[-1]

            if exec_step.status == StepStatus.FAILED:
                typer.echo("Execute failed.", err=True)
                raise typer.Exit(code=1)

            if not exec_step.result_json:
                typer.echo("Execute step has no result.", err=True)
                raise typer.Exit(code=1)
            exec_result = ExecuteResult.model_validate_json(exec_step.result_json)
            typer.echo(f"outcome: {exec_result.outcome.value}")
            typer.echo(f"signal: {exec_result.signal}")
            typer.echo(f"exit_code: {exec_result.exit_code}")
            typer.echo(f"duration_secs: {exec_result.duration_secs}")
            if exec_result.token_usage:
                typer.echo(f"token_cost: ${getattr(exec_result.token_usage, 'total_cost', 0):.4f}")
                typer.echo(f"token_total: {getattr(exec_result.token_usage, 'total_tokens', 0)}")
        finally:
            if hasattr(execution_env, "close"):
                await execution_env.close()
            await store.close()

    asyncio.run(_run())


@run_app.command("teardown")
def run_teardown(
    dispatch_id: Annotated[str, typer.Option(..., "--dispatch-id")],
) -> None:
    """Teardown a previously provisioned environment."""

    async def _run() -> None:
        config = _load_config()

        db_path = _store_path(config)
        store = await create_sqlite_store(db_path)
        execution_env = None

        try:
            steps = await store.get_steps_for_dispatch(dispatch_id)
            provision_step = next(
                (
                    s
                    for s in steps
                    if s.step_type == StepType.PROVISION and s.status == StepStatus.COMPLETED
                ),
                None,
            )
            if provision_step is None or provision_step.result_json is None:
                typer.echo(
                    f"No completed provision found for dispatch {dispatch_id}",
                    err=True,
                )
                raise typer.Exit(code=1)

            prov_result = ProvisionResult.model_validate_json(provision_step.result_json)
            view = await store.get_dispatch(dispatch_id)
            if view is None:
                typer.echo(f"Dispatch {dispatch_id} not found", err=True)
                raise typer.Exit(code=1)

            dispatch_data = view.dispatch
            profile = dispatch_data.resolved_profile
            env_factory, execution_env, _vm_store = _make_env_factory(config, profile)

            step_id = uuid.uuid4().hex
            payload = TeardownStepPayload(dispatch=dispatch_data, handle=prov_result.handle)
            await store.enqueue_step(
                step_id=step_id,
                dispatch_id=dispatch_id,
                step_type="teardown",
                step_sequence=2,
                lane=None,
                payload_json=payload.model_dump_json(),
            )

            worker = Worker(
                config=config,
                event_store=store,
                job_queue=store,
                state_store=store,
                env_factory=env_factory,
            )

            # Run until teardown completes
            await worker.run_until_step_complete(dispatch_id, StepType.TEARDOWN)

            typer.echo(f"teardown: completed for {dispatch_id}")
        finally:
            if execution_env is not None and hasattr(execution_env, "close"):
                await execution_env.close()
            await store.close()

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
        profile = _resolve_profile(config, project, environment_profile)

        # Use temp DB for single-invocation full lifecycle
        tmp_dir = Path(tempfile.mkdtemp(prefix="tanren-run-"))
        db_path = str(tmp_dir / "run.db")
        store = await create_sqlite_store(db_path)
        env_factory, execution_env, _vm_store = _make_env_factory(config, profile)

        try:
            workflow_id = (
                f"wf-{project}-cli-{uuid.uuid4().hex[:6]}-{int(datetime.now(UTC).timestamp())}"
            )
            tool = _resolve_agent_tool(config, phase)
            resolved_gate_cmd = _resolve_gate_cmd_for_phase(
                config=config,
                project=project,
                environment_profile=environment_profile,
                phase=phase,
                gate_cmd=gate_cmd,
            )
            project_env = _resolve_project_env(config, project)
            cloud_secrets = await _resolve_cloud_secrets(config, project)
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
                resolved_profile=profile,
                project_env=project_env,
                cloud_secrets=cloud_secrets,
            )

            dispatch_id = await _enqueue_dispatch(store, dispatch, DispatchMode.AUTO)

            worker = Worker(
                config=config,
                event_store=store,
                job_queue=store,
                state_store=store,
                env_factory=env_factory,
            )

            await worker.run_until_dispatch_complete(dispatch_id)

            # Read final state
            view = await store.get_dispatch(dispatch_id)
            steps = await store.get_steps_for_dispatch(dispatch_id)

            # Extract execute result
            exec_step = next(
                (s for s in steps if s.step_type == StepType.EXECUTE and s.result_json),
                None,
            )
            if exec_step and exec_step.result_json:
                exec_result = ExecuteResult.model_validate_json(exec_step.result_json)
                typer.echo(f"outcome: {exec_result.outcome.value}")
                typer.echo(f"signal: {exec_result.signal}")
                typer.echo(f"exit_code: {exec_result.exit_code}")
                typer.echo(f"duration_secs: {exec_result.duration_secs}")
                if exec_result.token_usage:
                    typer.echo(
                        f"token_cost: ${getattr(exec_result.token_usage, 'total_cost', 0):.4f}"
                    )
                    typer.echo(
                        f"token_total: {getattr(exec_result.token_usage, 'total_tokens', 0)}"
                    )

            # Extract provision result for VM info
            prov_step = next(
                (s for s in steps if s.step_type == StepType.PROVISION and s.result_json),
                None,
            )
            if prov_step and prov_step.result_json:
                prov_result = ProvisionResult.model_validate_json(prov_step.result_json)
                if prov_result.handle.vm:
                    typer.echo(f"provisioned_vm_id: {prov_result.handle.vm.vm_id}")

            typer.echo("teardown: completed")

            if view and view.status.value == "failed":
                raise typer.Exit(code=1)
        finally:
            if hasattr(execution_env, "close"):
                await execution_env.close()
            await store.close()
            # Clean up temp DB
            shutil.rmtree(tmp_dir, ignore_errors=True)

    asyncio.run(_run())


# Backward-compatible import name.
run = run_app
