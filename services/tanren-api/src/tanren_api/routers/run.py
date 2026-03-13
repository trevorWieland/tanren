"""Run lifecycle endpoints — provision, execute, teardown, full."""
# ruff: noqa: DOC201,DOC501

from __future__ import annotations

import asyncio
import contextlib
import logging
import uuid
from datetime import UTC, datetime
from typing import Annotated

from fastapi import APIRouter, Depends, Path

from tanren_api.dependencies import get_api_store, get_config, get_execution_env
from tanren_api.errors import ConflictError, NotFoundError, ServiceError
from tanren_api.models import (
    DispatchAccepted,
    DispatchRunStatus,
    ExecuteRequest,
    ProvisionRequest,
    RunEnvironment,
    RunEnvironmentStatus,
    RunExecuteAccepted,
    RunFullRequest,
    RunStatus,
    RunTeardownAccepted,
)
from tanren_api.state import APIStateStore, DispatchRecord, EnvironmentRecord
from tanren_core.adapters.protocols import ExecutionEnvironment
from tanren_core.adapters.types import EnvironmentHandle, RemoteEnvironmentRuntime
from tanren_core.config import Config
from tanren_core.schemas import Cli, Dispatch, Outcome, Phase

logger = logging.getLogger(__name__)

router = APIRouter(tags=["run"])

_EXECUTE_FROM = frozenset({RunEnvironmentStatus.PROVISIONED, RunEnvironmentStatus.COMPLETED})
_TEARDOWN_FROM = frozenset({
    RunEnvironmentStatus.PROVISIONED,
    RunEnvironmentStatus.EXECUTING,
    RunEnvironmentStatus.COMPLETED,
    RunEnvironmentStatus.FAILED,
})

_COMPLETED_DISPATCH_OUTCOMES = frozenset({Outcome.SUCCESS, Outcome.FAIL, Outcome.BLOCKED})
_COMPLETED_ENV_OUTCOMES = frozenset({Outcome.SUCCESS, Outcome.FAIL, Outcome.BLOCKED})


def _now() -> str:
    return datetime.now(UTC).isoformat()


@router.post("/run/provision")
async def run_provision(
    body: ProvisionRequest,
    store: Annotated[APIStateStore, Depends(get_api_store)],
    execution_env: Annotated[ExecutionEnvironment | None, Depends(get_execution_env)],
    config: Annotated[Config, Depends(get_config)],
) -> RunEnvironment:
    """Provision a remote execution environment."""
    if execution_env is None:
        raise ServiceError("Remote execution environment not configured")

    dispatch = Dispatch(
        workflow_id=f"run-{uuid.uuid4().hex[:8]}",
        project=body.project,
        phase=Phase.DO_TASK,
        branch=body.branch,
        spec_folder="",
        cli=Cli.CLAUDE,
        timeout=1800,
        environment_profile=body.environment_profile,
    )

    try:
        handle = await execution_env.provision(dispatch, config)
    except Exception as exc:
        logger.exception("Provision failed for project %s", body.project)
        raise ServiceError("Failed to provision environment") from exc
    if not isinstance(handle.runtime, RemoteEnvironmentRuntime):
        raise ServiceError("Provisioned environment is not a remote runtime")
    vm_handle = handle.runtime.vm_handle

    record = EnvironmentRecord(
        env_id=handle.env_id,
        handle=handle,
        status=RunEnvironmentStatus.PROVISIONED,
        vm_id=vm_handle.vm_id,
        host=vm_handle.host,
        started_at=_now(),
    )
    await store.add_environment(record)

    return RunEnvironment(
        env_id=handle.env_id,
        vm_id=vm_handle.vm_id,
        host=vm_handle.host,
        status=RunEnvironmentStatus.PROVISIONED,
    )


@router.post("/run/{env_id}/execute")
async def run_execute(
    env_id: Annotated[str, Path(description="Environment identifier")],
    body: ExecuteRequest,
    store: Annotated[APIStateStore, Depends(get_api_store)],
    execution_env: Annotated[ExecutionEnvironment | None, Depends(get_execution_env)],
    config: Annotated[Config, Depends(get_config)],
) -> RunExecuteAccepted:
    """Execute a phase against a provisioned environment."""
    record = await store.get_environment(env_id)
    if record is None:
        raise NotFoundError(f"Environment {env_id} not found")

    if execution_env is None:
        raise ServiceError("Remote execution environment not configured")

    if body.project != record.handle.project:
        raise ConflictError(
            f"Project mismatch: environment provisioned for '{record.handle.project}', "
            f"request specifies '{body.project}'"
        )

    dispatch_id = f"exec-{uuid.uuid4().hex[:8]}"

    dispatch = Dispatch(
        workflow_id=dispatch_id,
        project=body.project,
        phase=body.phase,
        branch=record.handle.branch,
        spec_folder=body.spec_path,
        cli=body.cli,
        model=body.model,
        timeout=body.timeout,
        context=body.context,
        gate_cmd=body.gate_cmd,
    )

    gate = asyncio.Event()

    async def _execute_background() -> None:
        await gate.wait()
        try:
            result = await execution_env.execute(record.handle, dispatch, config)
            env_status = (
                RunEnvironmentStatus.COMPLETED
                if result.outcome in _COMPLETED_ENV_OUTCOMES
                else RunEnvironmentStatus.FAILED
            )
            await store.update_environment(
                env_id,
                status=env_status,
                outcome=result.outcome,
                completed_at=_now(),
            )
        except asyncio.CancelledError:
            raise
        except Exception:
            logger.exception("Execute failed for env %s", env_id)
            await store.update_environment(
                env_id,
                status=RunEnvironmentStatus.FAILED,
                outcome=Outcome.ERROR,
                completed_at=_now(),
            )

    task = asyncio.create_task(_execute_background())
    updated = await store.try_transition_environment(
        env_id,
        from_statuses=_EXECUTE_FROM,
        to_status=RunEnvironmentStatus.EXECUTING,
        phase=body.phase,
        dispatch_id=dispatch_id,
        started_at=_now(),
        outcome=None,
        completed_at=None,
        task=task,
    )
    if updated is None:
        task.cancel()
        with contextlib.suppress(asyncio.CancelledError):
            await task
        raise ConflictError(f"Environment {env_id} cannot transition to executing")
    gate.set()

    return RunExecuteAccepted(env_id=env_id, dispatch_id=dispatch_id)


@router.post("/run/{env_id}/teardown")
async def run_teardown(
    env_id: Annotated[str, Path(description="Environment identifier")],
    store: Annotated[APIStateStore, Depends(get_api_store)],
    execution_env: Annotated[ExecutionEnvironment | None, Depends(get_execution_env)],
) -> RunTeardownAccepted:
    """Teardown a provisioned environment."""
    record = await store.get_environment(env_id)
    if record is None:
        raise NotFoundError(f"Environment {env_id} not found")

    if execution_env is None:
        raise ServiceError("Remote execution environment not configured")

    # Cancel any running execute task and wait for it to finish
    stopped = await store.cancel_environment_task(env_id)
    if not stopped:
        raise ConflictError(
            f"Environment {env_id} has a running task that could not be stopped. Please retry."
        )

    updated = await store.try_transition_environment(
        env_id,
        from_statuses=_TEARDOWN_FROM,
        to_status=RunEnvironmentStatus.TEARING_DOWN,
    )
    if updated is None:
        raise ConflictError(f"Environment {env_id} is already being torn down")

    async def _teardown_background() -> None:
        try:
            await execution_env.teardown(record.handle)
        except Exception:
            logger.warning("Teardown failed for env %s", env_id)
        finally:
            await store.remove_environment(env_id)

    task = asyncio.create_task(_teardown_background())
    await store.update_environment(env_id, task=task)

    return RunTeardownAccepted(env_id=env_id)


@router.post("/run/full")
async def run_full(
    body: RunFullRequest,
    store: Annotated[APIStateStore, Depends(get_api_store)],
    execution_env: Annotated[ExecutionEnvironment | None, Depends(get_execution_env)],
    config: Annotated[Config, Depends(get_config)],
) -> DispatchAccepted:
    """Full lifecycle: provision, execute, teardown. Returns ID for polling."""
    if execution_env is None:
        raise ServiceError("Remote execution environment not configured")

    workflow_id = f"full-{uuid.uuid4().hex[:8]}"

    dispatch = Dispatch(
        workflow_id=workflow_id,
        project=body.project,
        phase=body.phase,
        branch=body.branch,
        spec_folder=body.spec_path,
        cli=Cli.CLAUDE,
        timeout=body.timeout,
        environment_profile=body.environment_profile,
        context=body.context,
        gate_cmd=body.gate_cmd,
    )

    async def _full_lifecycle() -> None:
        handle: EnvironmentHandle | None = None
        try:
            handle = await execution_env.provision(dispatch, config)
            if not isinstance(handle.runtime, RemoteEnvironmentRuntime):
                raise ServiceError("Provisioned environment is not a remote runtime")
            vm_handle = handle.runtime.vm_handle

            env_record = EnvironmentRecord(
                env_id=handle.env_id,
                handle=handle,
                status=RunEnvironmentStatus.EXECUTING,
                phase=body.phase,
                vm_id=vm_handle.vm_id,
                host=vm_handle.host,
                dispatch_id=workflow_id,
                started_at=_now(),
            )
            await store.add_environment(env_record)

            result = await execution_env.execute(handle, dispatch, config)
            dispatch_status = (
                DispatchRunStatus.COMPLETED
                if result.outcome in _COMPLETED_DISPATCH_OUTCOMES
                else DispatchRunStatus.FAILED
            )
            env_status = (
                RunEnvironmentStatus.COMPLETED
                if result.outcome in _COMPLETED_ENV_OUTCOMES
                else RunEnvironmentStatus.FAILED
            )
            await store.try_transition_dispatch(
                workflow_id,
                from_statuses=frozenset({DispatchRunStatus.RUNNING}),
                to_status=dispatch_status,
                outcome=result.outcome,
                completed_at=_now(),
            )
            await store.update_environment(
                handle.env_id,
                status=env_status,
                outcome=result.outcome,
                completed_at=_now(),
            )
        except asyncio.CancelledError:
            raise
        except Exception:
            logger.exception("Full lifecycle failed for %s", workflow_id)
            await store.try_transition_dispatch(
                workflow_id,
                from_statuses=frozenset({DispatchRunStatus.RUNNING}),
                to_status=DispatchRunStatus.FAILED,
                outcome=Outcome.ERROR,
                completed_at=_now(),
            )
        finally:
            if handle is not None:
                try:
                    await asyncio.shield(execution_env.teardown(handle))
                except asyncio.CancelledError, Exception:
                    logger.warning("Teardown failed for %s", workflow_id)
                finally:
                    try:
                        await store.remove_environment(handle.env_id)
                    except asyncio.CancelledError, Exception:
                        logger.warning("Failed to remove environment %s", handle.env_id)

    dispatch_record = DispatchRecord(
        dispatch_id=workflow_id,
        dispatch=dispatch,
        status=DispatchRunStatus.RUNNING,
        created_at=_now(),
        started_at=_now(),
    )
    await store.add_dispatch(dispatch_record)

    task = asyncio.create_task(_full_lifecycle())
    await store.update_dispatch(workflow_id, task=task)

    return DispatchAccepted(dispatch_id=workflow_id)


@router.get("/run/{env_id}/status")
async def run_status(
    env_id: Annotated[str, Path(description="Environment identifier")],
    store: Annotated[APIStateStore, Depends(get_api_store)],
) -> RunStatus:
    """Poll status of a running environment."""
    record = await store.get_environment(env_id)
    if record is None:
        raise NotFoundError(f"Environment {env_id} not found")

    duration_secs = None
    if record.started_at is not None:
        try:
            started = datetime.fromisoformat(record.started_at)
            duration_secs = int((datetime.now(UTC) - started).total_seconds())
        except ValueError, TypeError:
            pass

    return RunStatus(
        env_id=env_id,
        status=record.status,
        phase=record.phase,
        outcome=record.outcome,
        started_at=record.started_at,
        duration_secs=duration_secs,
    )
