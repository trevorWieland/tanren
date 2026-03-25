"""Run lifecycle endpoints — provision, execute, teardown, full."""

from __future__ import annotations

from typing import Annotated

from fastapi import APIRouter, Depends, Path

from tanren_api.dependencies import get_event_store, get_job_queue, get_state_store
from tanren_api.models import (
    DispatchAccepted,
    ExecuteRequest,
    ProvisionRequest,
    RunEnvironment,
    RunExecuteAccepted,
    RunFullRequest,
    RunStatus,
    RunTeardownAccepted,
)
from tanren_api.services.run import RunService
from tanren_core.store.protocols import EventStore, JobQueue, StateStore

router = APIRouter(tags=["run"])


def _run_service(
    event_store: EventStore, job_queue: JobQueue, state_store: StateStore
) -> RunService:
    return RunService(event_store=event_store, job_queue=job_queue, state_store=state_store)


@router.post("/run/provision")
async def run_provision(
    body: ProvisionRequest,
    event_store: Annotated[EventStore, Depends(get_event_store)],
    job_queue: Annotated[JobQueue, Depends(get_job_queue)],
    state_store: Annotated[StateStore, Depends(get_state_store)],
) -> RunEnvironment:
    """Provision a remote execution environment (non-blocking).

    Returns:
        RunEnvironment: Provisioning environment with tracking env_id.
    """
    return await _run_service(event_store, job_queue, state_store).provision(body)


@router.post("/run/{env_id}/execute")
async def run_execute(
    env_id: Annotated[str, Path(description="Environment identifier")],
    body: ExecuteRequest,
    event_store: Annotated[EventStore, Depends(get_event_store)],
    job_queue: Annotated[JobQueue, Depends(get_job_queue)],
    state_store: Annotated[StateStore, Depends(get_state_store)],
) -> RunExecuteAccepted:
    """Execute a phase against a provisioned environment.

    Returns:
        RunExecuteAccepted: Accepted response with env_id and dispatch_id.
    """
    return await _run_service(event_store, job_queue, state_store).execute(env_id, body)


@router.post("/run/{env_id}/teardown")
async def run_teardown(
    env_id: Annotated[str, Path(description="Environment identifier")],
    event_store: Annotated[EventStore, Depends(get_event_store)],
    job_queue: Annotated[JobQueue, Depends(get_job_queue)],
    state_store: Annotated[StateStore, Depends(get_state_store)],
) -> RunTeardownAccepted:
    """Teardown a provisioned environment.

    Returns:
        RunTeardownAccepted: Confirmation that teardown has been initiated.
    """
    return await _run_service(event_store, job_queue, state_store).teardown(env_id)


@router.post("/run/full")
async def run_full(
    body: RunFullRequest,
    event_store: Annotated[EventStore, Depends(get_event_store)],
    job_queue: Annotated[JobQueue, Depends(get_job_queue)],
    state_store: Annotated[StateStore, Depends(get_state_store)],
) -> DispatchAccepted:
    """Full lifecycle: provision, execute, teardown. Returns ID for polling.

    Returns:
        DispatchAccepted: Accepted response with dispatch_id for polling.
    """
    return await _run_service(event_store, job_queue, state_store).full(body)


@router.get("/run/{env_id}/status")
async def run_status(
    env_id: Annotated[str, Path(description="Environment identifier")],
    event_store: Annotated[EventStore, Depends(get_event_store)],
    job_queue: Annotated[JobQueue, Depends(get_job_queue)],
    state_store: Annotated[StateStore, Depends(get_state_store)],
) -> RunStatus:
    """Poll status of a running environment.

    Returns:
        RunStatus: Current status of the environment.
    """
    return await _run_service(event_store, job_queue, state_store).status(env_id)
