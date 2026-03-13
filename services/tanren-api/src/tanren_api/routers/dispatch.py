"""Dispatch endpoints — accept, query, and cancel dispatch requests."""
# ruff: noqa: DOC201,DOC501

from __future__ import annotations

import asyncio
import logging
import time
from datetime import UTC, datetime
from pathlib import Path
from typing import Annotated

from fastapi import APIRouter, Depends
from fastapi import Path as PathParam

from tanren_api.dependencies import get_api_store, get_config, get_emitter, get_execution_env
from tanren_api.errors import ConflictError, NotFoundError
from tanren_api.models import (
    DispatchAccepted,
    DispatchCancelled,
    DispatchDetail,
    DispatchRequest,
    DispatchRunStatus,
)
from tanren_api.state import APIStateStore, DispatchRecord
from tanren_core.adapters.events import DispatchReceived
from tanren_core.adapters.protocols import EventEmitter, ExecutionEnvironment
from tanren_core.config import Config
from tanren_core.ipc import atomic_write, generate_filename
from tanren_core.schemas import Dispatch, Outcome

logger = logging.getLogger(__name__)

router = APIRouter(tags=["dispatch"])


def _now() -> str:
    return datetime.now(UTC).isoformat()


async def _dispatch_background(
    dispatch: Dispatch,
    dispatch_id: str,
    execution_env: ExecutionEnvironment,
    config: Config,
    store: APIStateStore,
) -> None:
    """Background task: provision -> execute -> teardown."""
    await store.update_dispatch(dispatch_id, status=DispatchRunStatus.RUNNING, started_at=_now())
    handle = None
    try:
        handle = await execution_env.provision(dispatch, config)
        result = await execution_env.execute(handle, dispatch, config)
        await store.update_dispatch(
            dispatch_id,
            status=DispatchRunStatus.COMPLETED,
            outcome=result.outcome,
            completed_at=_now(),
        )
    except asyncio.CancelledError:
        logger.info("Dispatch %s cancelled", dispatch_id)
        raise
    except Exception:
        logger.exception("Dispatch %s failed", dispatch_id)
        await store.update_dispatch(
            dispatch_id,
            status=DispatchRunStatus.FAILED,
            outcome=Outcome.ERROR,
            completed_at=_now(),
        )
    finally:
        if handle is not None:
            try:
                await execution_env.teardown(handle)
            except Exception:
                logger.warning("Teardown failed for dispatch %s", dispatch_id)


@router.post("/dispatch")
async def create_dispatch(
    body: DispatchRequest,
    store: Annotated[APIStateStore, Depends(get_api_store)],
    config: Annotated[Config, Depends(get_config)],
    emitter: Annotated[EventEmitter, Depends(get_emitter)],
    execution_env: Annotated[ExecutionEnvironment | None, Depends(get_execution_env)],
) -> DispatchAccepted:
    """Accept a new dispatch request."""
    epoch = int(time.time())
    workflow_id = f"wf-{body.project}-{body.issue}-{epoch}"

    dispatch = Dispatch(
        workflow_id=workflow_id,
        project=body.project,
        phase=body.phase,
        branch=body.branch,
        spec_folder=body.spec_folder,
        cli=body.cli,
        model=body.model,
        timeout=body.timeout,
        environment_profile=body.environment_profile,
        context=body.context,
        gate_cmd=body.gate_cmd,
    )

    # Write dispatch to IPC directory
    dispatch_dir = Path(config.ipc_dir) / "dispatch"
    dispatch_dir.mkdir(parents=True, exist_ok=True)
    dispatch_path = dispatch_dir / generate_filename()
    await atomic_write(dispatch_path, dispatch.model_dump_json(indent=2))

    # Register in state store
    record = DispatchRecord(
        dispatch_id=workflow_id,
        dispatch=dispatch,
        status=DispatchRunStatus.PENDING,
        created_at=_now(),
    )
    record.dispatch_path = dispatch_path
    await store.add_dispatch(record)

    # Emit event
    await emitter.emit(
        DispatchReceived(
            timestamp=_now(),
            workflow_id=workflow_id,
            phase=body.phase.value,
            project=body.project,
            cli=body.cli.value,
        )
    )

    # Launch background task if execution env available
    if execution_env is not None:
        task = asyncio.create_task(
            _dispatch_background(dispatch, workflow_id, execution_env, config, store)
        )
        await store.update_dispatch(workflow_id, task=task)

    return DispatchAccepted(dispatch_id=workflow_id)


@router.get("/dispatch/{dispatch_id}")
async def get_dispatch(
    dispatch_id: Annotated[str, PathParam(description="Workflow ID")],
    store: Annotated[APIStateStore, Depends(get_api_store)],
) -> DispatchDetail:
    """Query dispatch status by workflow ID."""
    record = await store.get_dispatch(dispatch_id)
    if record is None:
        raise NotFoundError(f"Dispatch {dispatch_id} not found")

    d = record.dispatch
    return DispatchDetail(
        workflow_id=d.workflow_id,
        phase=d.phase,
        project=d.project,
        spec_folder=d.spec_folder,
        branch=d.branch,
        cli=d.cli,
        model=d.model,
        timeout=d.timeout,
        environment_profile=d.environment_profile,
        context=d.context,
        gate_cmd=d.gate_cmd,
        status=record.status,
        outcome=record.outcome,
        created_at=record.created_at,
        started_at=record.started_at,
        completed_at=record.completed_at,
    )


@router.delete("/dispatch/{dispatch_id}")
async def cancel_dispatch(
    dispatch_id: Annotated[str, PathParam(description="Workflow ID")],
    store: Annotated[APIStateStore, Depends(get_api_store)],
) -> DispatchCancelled:
    """Cancel a pending dispatch."""
    record = await store.get_dispatch(dispatch_id)
    if record is None:
        raise NotFoundError(f"Dispatch {dispatch_id} not found")

    if record.status in (
        DispatchRunStatus.COMPLETED,
        DispatchRunStatus.FAILED,
        DispatchRunStatus.CANCELLED,
    ):
        raise ConflictError(f"Dispatch {dispatch_id} is already {record.status}")

    if record.status == DispatchRunStatus.PENDING:
        await store.update_dispatch(
            dispatch_id,
            status=DispatchRunStatus.CANCELLED,
            completed_at=_now(),
        )
    elif record.status == DispatchRunStatus.RUNNING:
        logger.warning("Cancelling running dispatch %s — best effort", dispatch_id)
        if record.task is not None and not record.task.done():
            record.task.cancel()
        await store.update_dispatch(
            dispatch_id,
            status=DispatchRunStatus.CANCELLED,
            completed_at=_now(),
        )

    # Remove IPC dispatch file to prevent daemon pickup
    if record.dispatch_path is not None:
        try:
            record.dispatch_path.unlink(missing_ok=True)
        except OSError:
            logger.debug("Could not remove dispatch file for %s", dispatch_id)

    return DispatchCancelled(dispatch_id=dispatch_id)
