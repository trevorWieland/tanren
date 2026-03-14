"""Dispatch endpoints — accept, query, and cancel dispatch requests."""
# ruff: noqa: DOC201,DOC501

from __future__ import annotations

import asyncio
import contextlib
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
from tanren_core.schemas import Dispatch, Outcome, Result

logger = logging.getLogger(__name__)

router = APIRouter(tags=["dispatch"])

_COMPLETED_OUTCOMES = frozenset({Outcome.SUCCESS, Outcome.FAIL, Outcome.BLOCKED})


def _status_for_outcome(outcome: Outcome) -> DispatchRunStatus:
    """Map execution outcome to terminal dispatch status."""
    if outcome in _COMPLETED_OUTCOMES:
        return DispatchRunStatus.COMPLETED
    return DispatchRunStatus.FAILED


def _now() -> str:
    return datetime.now(UTC).isoformat()


async def _check_daemon_result(
    config: Config, workflow_id: str, store: APIStateStore, created_at: str
) -> None:
    """Scan IPC results directory for a daemon-produced result matching workflow_id."""
    results_dir = Path(config.ipc_dir) / "results"
    if not results_dir.exists():
        return

    # Parse created_at to millisecond timestamp for filename filtering
    try:
        created_dt = datetime.fromisoformat(created_at)
        created_ms = int(created_dt.timestamp() * 1000)
    except ValueError, TypeError:
        created_ms = 0

    def _scan() -> Result | None:
        entries = sorted(results_dir.iterdir(), reverse=True)
        for entry in entries:
            if entry.suffix != ".json":
                continue
            # Skip files older than dispatch creation
            stem = entry.stem.split("-")[0]
            try:
                file_ts = int(stem)
                if file_ts < created_ms:
                    break  # Sorted newest-first — all remaining are older
            except ValueError, IndexError:
                pass
            try:
                content = entry.read_text()
                result = Result.model_validate_json(content)
                if result.workflow_id == workflow_id:
                    return result
            except Exception:
                continue
        return None

    result = await asyncio.to_thread(_scan)
    if result is None:
        return

    await store.update_dispatch(
        workflow_id,
        status=_status_for_outcome(result.outcome),
        outcome=result.outcome,
        completed_at=_now(),
    )


async def _dispatch_background(
    dispatch: Dispatch,
    dispatch_id: str,
    execution_env: ExecutionEnvironment,
    config: Config,
    store: APIStateStore,
) -> None:
    """Background task: provision -> execute -> teardown."""
    transitioned = await store.try_transition_dispatch(
        dispatch_id,
        from_statuses=frozenset({DispatchRunStatus.PENDING}),
        to_status=DispatchRunStatus.RUNNING,
        started_at=_now(),
    )
    if transitioned is None:
        return  # Cancelled (or otherwise transitioned) before we started
    handle = None
    try:
        handle = await execution_env.provision(dispatch, config)
        result = await execution_env.execute(handle, dispatch, config)
        await store.try_transition_dispatch(
            dispatch_id,
            from_statuses=frozenset({DispatchRunStatus.RUNNING}),
            to_status=_status_for_outcome(result.outcome),
            outcome=result.outcome,
            completed_at=_now(),
        )
    except asyncio.CancelledError:
        logger.info("Dispatch %s cancelled", dispatch_id)
        raise
    except Exception:
        logger.exception("Dispatch %s failed", dispatch_id)
        await store.try_transition_dispatch(
            dispatch_id,
            from_statuses=frozenset({DispatchRunStatus.RUNNING}),
            to_status=DispatchRunStatus.FAILED,
            outcome=Outcome.ERROR,
            completed_at=_now(),
        )
    finally:
        if handle is not None:
            inner = asyncio.ensure_future(execution_env.teardown(handle))
            try:
                await asyncio.shield(inner)
            except asyncio.CancelledError, Exception:
                with contextlib.suppress(asyncio.CancelledError, Exception):
                    await inner
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
    epoch = time.time_ns()
    issue = body.issue if body.issue != 0 else epoch
    workflow_id = f"wf-{body.project}-{issue}-{epoch}"

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

    # Write dispatch to IPC only when delegating to daemon (no local execution env)
    dispatch_path = None
    if execution_env is None:
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

    # Launch background task if execution env available
    if execution_env is not None:
        # Emit event — daemon-delegated dispatches emit on pickup instead
        await emitter.emit(
            DispatchReceived(
                timestamp=_now(),
                workflow_id=workflow_id,
                phase=body.phase.value,
                project=body.project,
                cli=body.cli.value,
            )
        )
        task = asyncio.create_task(
            _dispatch_background(dispatch, workflow_id, execution_env, config, store)
        )
        await store.update_dispatch(workflow_id, task=task)

    return DispatchAccepted(dispatch_id=workflow_id)


@router.get("/dispatch/{dispatch_id}")
async def get_dispatch(
    dispatch_id: Annotated[str, PathParam(description="Workflow ID")],
    store: Annotated[APIStateStore, Depends(get_api_store)],
    config: Annotated[Config, Depends(get_config)],
) -> DispatchDetail:
    """Query dispatch status by workflow ID."""
    record = await store.get_dispatch(dispatch_id)
    if record is None:
        raise NotFoundError(f"Dispatch {dispatch_id} not found")

    # Lazy-check daemon results for IPC-delegated dispatches still pending
    if (
        record.status == DispatchRunStatus.PENDING
        and record.dispatch_path is not None
        and record.task is None
    ):
        await _check_daemon_result(config, dispatch_id, store, record.created_at)
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
        # For daemon-delegated: reclaim IPC file before marking cancelled.
        # If the daemon already picked it up (deleted), we cannot cancel.
        # Narrow race: daemon may have read but not yet deleted — microseconds
        # between scan and delete in the poll loop, acceptable.
        if record.dispatch_path is not None and record.task is None:
            try:
                record.dispatch_path.unlink()
            except FileNotFoundError:
                raise ConflictError(
                    f"Dispatch {dispatch_id} has been picked up by the daemon "
                    "and cannot be cancelled"
                ) from None
            except OSError:
                raise ConflictError(
                    f"Dispatch {dispatch_id} could not be cancelled: "
                    "failed to remove dispatch file. Please retry."
                ) from None
        transitioned = await store.try_transition_dispatch(
            dispatch_id,
            from_statuses=frozenset({DispatchRunStatus.PENDING}),
            to_status=DispatchRunStatus.CANCELLED,
            completed_at=_now(),
        )
        if transitioned is None:
            raise ConflictError(f"Dispatch {dispatch_id} status has changed; cannot cancel")
        if record.task is not None and not record.task.done():
            record.task.cancel()
    elif record.status == DispatchRunStatus.RUNNING:
        logger.warning("Cancelling running dispatch %s — best effort", dispatch_id)
        transitioned = await store.try_transition_dispatch(
            dispatch_id,
            from_statuses=frozenset({DispatchRunStatus.RUNNING}),
            to_status=DispatchRunStatus.CANCELLED,
            completed_at=_now(),
        )
        if transitioned is None:
            raise ConflictError(f"Dispatch {dispatch_id} status has changed; cannot cancel")
        if record.task is not None and not record.task.done():
            record.task.cancel()

    return DispatchCancelled(dispatch_id=dispatch_id)
