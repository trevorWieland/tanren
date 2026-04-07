"""tanren run - standalone execution lifecycle commands.

Reference docs:
- docs/interfaces.md
- docs/getting-started/bootstrap.md
"""

from __future__ import annotations

import asyncio
import uuid
from datetime import UTC, datetime
from pathlib import Path
from typing import TYPE_CHECKING, Annotated

import typer

from tanren_core.builder import build_execution_environment
from tanren_core.config_resolver import DiskConfigResolver
from tanren_core.dispatch_builder import (
    ResolvedInputs,
    resolve_dispatch_inputs,
    resolve_provision_inputs,
)
from tanren_core.dispatch_orchestrator import (
    DispatchGuardError,
    create_dispatch,
    enqueue_execute_step,
    enqueue_teardown_step,
    get_provision_result,
)
from tanren_core.schemas import Dispatch, Phase
from tanren_core.store.enums import DispatchMode, StepStatus, StepType
from tanren_core.store.factory import create_sqlite_store
from tanren_core.store.payloads import (
    ExecuteResult,
    ProvisionResult,
)
from tanren_core.worker import Worker
from tanren_core.worker_config import WorkerConfig

if TYPE_CHECKING:
    from tanren_core.env.environment_schema import EnvironmentProfile

run_app = typer.Typer(help="Run provision/execute/teardown lifecycle without coordinator.")


def _load_config() -> WorkerConfig:
    try:
        return WorkerConfig.from_env()
    except Exception as exc:
        typer.echo(f"Failed to load config from WM_* environment: {exc}", err=True)
        raise typer.Exit(code=1) from exc


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
    resolved: ResolvedInputs,
) -> Dispatch:
    return Dispatch(
        workflow_id=workflow_id,
        phase=phase,
        project=project,
        spec_folder=spec_path,
        branch=branch,
        cli=resolved.cli,
        auth=resolved.auth,
        model=resolved.model,
        gate_cmd=resolved.gate_cmd,
        context=context,
        timeout=timeout,
        environment_profile=environment_profile,
        resolved_profile=resolved.profile,
        project_env=resolved.project_env,
        cloud_secrets=resolved.cloud_secrets,
        required_secrets=resolved.required_secrets,
    )


def _store_path(config: WorkerConfig) -> str:
    """Return the persistent store path for multi-step CLI workflows."""
    return str(Path(config.data_dir) / "run.db")


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
        resolver = DiskConfigResolver(config.github_dir)

        try:
            resolved = await resolve_provision_inputs(
                resolver=resolver,
                config=config,
                project=project,
                branch=branch,
                environment_profile=environment_profile,
            )
        except ValueError as exc:
            typer.echo(str(exc), err=True)
            raise typer.Exit(code=1) from exc

        db_path = _store_path(config)
        store = await create_sqlite_store(db_path)
        env_factory, execution_env, _vm_store = _make_env_factory(config, resolved.profile)

        try:
            workflow_id = (
                f"wf-{project}-cli-{uuid.uuid4().hex[:6]}-{int(datetime.now(UTC).timestamp())}"
            )
            dispatch = _build_dispatch(
                project=project,
                phase=Phase.DO_TASK,
                spec_path=".",
                branch=branch,
                environment_profile=environment_profile,
                workflow_id=workflow_id,
                timeout=1800,
                context=None,
                resolved=resolved,
            )

            result = await create_dispatch(
                dispatch=dispatch,
                mode=DispatchMode.MANUAL,
                event_store=store,
                job_queue=store,
                state_store=store,
            )
            dispatch_id = result.dispatch_id

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
        execution_env = None

        try:
            # Get provision result via orchestrator helper
            try:
                prov_result = await get_provision_result(store, dispatch_id)
            except ValueError as exc:
                typer.echo(str(exc), err=True)
                raise typer.Exit(code=1) from exc

            view = await store.get_dispatch(dispatch_id)
            if view is None:
                typer.echo(f"Dispatch {dispatch_id} not found", err=True)
                raise typer.Exit(code=1)

            dispatch_data = view.dispatch
            profile = dispatch_data.resolved_profile
            env_factory, execution_env, _vm_store = _make_env_factory(config, profile)

            resolver = DiskConfigResolver(config.github_dir)
            try:
                resolved = await resolve_dispatch_inputs(
                    resolver=resolver,
                    config=config,
                    project=project,
                    phase=phase,
                    branch=dispatch_data.branch,
                    environment_profile=dispatch_data.environment_profile,
                    gate_cmd=gate_cmd,
                    # Profile already resolved from stored dispatch
                    resolved_profile=profile,
                    # Env/secrets already in stored dispatch — skip re-resolution
                    project_env={},
                    cloud_secrets={},
                )
            except ValueError as exc:
                typer.echo(str(exc), err=True)
                raise typer.Exit(code=1) from exc

            exec_dispatch = _build_dispatch(
                project=project,
                phase=phase,
                spec_path=spec_path,
                branch=dispatch_data.branch,
                environment_profile=dispatch_data.environment_profile,
                workflow_id=dispatch_id,
                timeout=timeout,
                context=context,
                resolved=resolved,
            )

            try:
                await enqueue_execute_step(
                    dispatch_id=dispatch_id,
                    exec_dispatch=exec_dispatch,
                    handle=prov_result.handle,
                    job_queue=store,
                    state_store=store,
                )
            except DispatchGuardError as exc:
                typer.echo(str(exc), err=True)
                raise typer.Exit(code=1) from exc

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
            # Get provision result via orchestrator helper
            try:
                prov_result = await get_provision_result(store, dispatch_id)
            except ValueError as exc:
                typer.echo(str(exc), err=True)
                raise typer.Exit(code=1) from exc

            view = await store.get_dispatch(dispatch_id)
            if view is None:
                typer.echo(f"Dispatch {dispatch_id} not found", err=True)
                raise typer.Exit(code=1)

            dispatch_data = view.dispatch
            profile = dispatch_data.resolved_profile
            env_factory, execution_env, _vm_store = _make_env_factory(config, profile)

            try:
                await enqueue_teardown_step(
                    dispatch_id=dispatch_id,
                    dispatch=dispatch_data,
                    handle=prov_result.handle,
                    job_queue=store,
                    state_store=store,
                )
            except DispatchGuardError as exc:
                typer.echo(str(exc), err=True)
                raise typer.Exit(code=1) from exc

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
        resolver = DiskConfigResolver(config.github_dir)

        try:
            resolved = await resolve_dispatch_inputs(
                resolver=resolver,
                config=config,
                project=project,
                phase=phase,
                branch=branch,
                environment_profile=environment_profile,
                gate_cmd=gate_cmd,
            )
        except ValueError as exc:
            typer.echo(str(exc), err=True)
            raise typer.Exit(code=1) from exc

        # Use persistent store so dispatches are auditable
        db_path = _store_path(config)
        store = await create_sqlite_store(db_path)
        env_factory, execution_env, _vm_store = _make_env_factory(config, resolved.profile)

        try:
            workflow_id = (
                f"wf-{project}-cli-{uuid.uuid4().hex[:6]}-{int(datetime.now(UTC).timestamp())}"
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
                resolved=resolved,
            )

            result = await create_dispatch(
                dispatch=dispatch,
                mode=DispatchMode.AUTO,
                event_store=store,
                job_queue=store,
                state_store=store,
            )
            dispatch_id = result.dispatch_id

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

            # Extract result from the latest execute step (not first)
            exec_steps = [s for s in steps if s.step_type == StepType.EXECUTE and s.result_json]
            exec_step = exec_steps[-1] if exec_steps else None
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

    asyncio.run(_run())


# Backward-compatible import name.
run = run_app
