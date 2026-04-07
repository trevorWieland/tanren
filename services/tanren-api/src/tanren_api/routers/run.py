"""Run lifecycle endpoints — provision, execute, teardown, full."""

from __future__ import annotations

from typing import Annotated

from fastapi import APIRouter, Depends, Path

from tanren_api.auth import require_scope
from tanren_api.dependencies import (
    get_auth_store,
    get_event_store,
    get_job_queue,
    get_state_store,
    get_store,
    get_worker_config,
)
from tanren_api.limits import check_resource_limits
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
from tanren_api.scopes import has_scope
from tanren_api.services.run import RunService
from tanren_core.store.auth_protocols import AuthStore
from tanren_core.store.auth_views import AuthContext
from tanren_core.store.protocols import EventStore, JobQueue, StateStore
from tanren_core.store.repository import Store
from tanren_core.worker_config import WorkerConfig

router = APIRouter(tags=["run"])


def _run_service(
    event_store: EventStore,
    job_queue: JobQueue,
    state_store: StateStore,
    config: WorkerConfig | None = None,
) -> RunService:
    return RunService(
        event_store=event_store, job_queue=job_queue, state_store=state_store, config=config
    )


@router.post("/run/provision")
async def run_provision(
    body: ProvisionRequest,
    auth: Annotated[AuthContext, Depends(require_scope("run:provision"))],
    event_store: Annotated[EventStore, Depends(get_event_store)],
    job_queue: Annotated[JobQueue, Depends(get_job_queue)],
    state_store: Annotated[StateStore, Depends(get_state_store)],
    auth_store: Annotated[AuthStore, Depends(get_auth_store)],
    store: Annotated[Store, Depends(get_store)],
) -> RunEnvironment:
    """Provision a remote execution environment (non-blocking).

    Returns:
        RunEnvironment: Provisioning environment with tracking env_id.
    """
    async with store.user_quota_lock(auth.user.user_id):
        await check_resource_limits(auth, auth_store, "dispatch")
        return await _run_service(event_store, job_queue, state_store).provision(
            body, user_id=auth.user.user_id
        )


@router.post("/run/{env_id}/execute")
async def run_execute(
    env_id: Annotated[str, Path(description="Environment identifier")],
    body: ExecuteRequest,
    auth: Annotated[AuthContext, Depends(require_scope("run:execute"))],
    event_store: Annotated[EventStore, Depends(get_event_store)],
    job_queue: Annotated[JobQueue, Depends(get_job_queue)],
    state_store: Annotated[StateStore, Depends(get_state_store)],
    worker_config: Annotated[WorkerConfig | None, Depends(get_worker_config)] = None,
) -> RunExecuteAccepted:
    """Execute a phase against a provisioned environment.

    Returns:
        RunExecuteAccepted: Accepted response with env_id and dispatch_id.
    """
    return await _run_service(event_store, job_queue, state_store, config=worker_config).execute(
        env_id, body, user_id=auth.user.user_id, is_admin=has_scope(auth.scopes, "admin:*")
    )


@router.post("/run/{env_id}/teardown")
async def run_teardown(
    env_id: Annotated[str, Path(description="Environment identifier")],
    auth: Annotated[AuthContext, Depends(require_scope("run:teardown"))],
    event_store: Annotated[EventStore, Depends(get_event_store)],
    job_queue: Annotated[JobQueue, Depends(get_job_queue)],
    state_store: Annotated[StateStore, Depends(get_state_store)],
) -> RunTeardownAccepted:
    """Teardown a provisioned environment.

    Returns:
        RunTeardownAccepted: Confirmation that teardown has been initiated.
    """
    return await _run_service(event_store, job_queue, state_store).teardown(
        env_id, user_id=auth.user.user_id, is_admin=has_scope(auth.scopes, "admin:*")
    )


@router.post("/run/full")
async def run_full(
    body: RunFullRequest,
    auth: Annotated[AuthContext, Depends(require_scope("run:full"))],
    event_store: Annotated[EventStore, Depends(get_event_store)],
    job_queue: Annotated[JobQueue, Depends(get_job_queue)],
    state_store: Annotated[StateStore, Depends(get_state_store)],
    auth_store: Annotated[AuthStore, Depends(get_auth_store)],
    store: Annotated[Store, Depends(get_store)],
) -> DispatchAccepted:
    """Full lifecycle: provision, execute, teardown. Returns ID for polling.

    Returns:
        DispatchAccepted: Accepted response with dispatch_id for polling.
    """
    async with store.user_quota_lock(auth.user.user_id):
        await check_resource_limits(auth, auth_store, "dispatch")
        return await _run_service(event_store, job_queue, state_store).full(
            body, user_id=auth.user.user_id
        )


@router.get("/run/{env_id}/status")
async def run_status(
    env_id: Annotated[str, Path(description="Environment identifier")],
    auth: Annotated[AuthContext, Depends(require_scope("run:read"))],
    event_store: Annotated[EventStore, Depends(get_event_store)],
    job_queue: Annotated[JobQueue, Depends(get_job_queue)],
    state_store: Annotated[StateStore, Depends(get_state_store)],
) -> RunStatus:
    """Poll status of a running environment.

    Returns:
        RunStatus: Current status of the environment.
    """
    return await _run_service(event_store, job_queue, state_store).status(
        env_id, user_id=auth.user.user_id, is_admin=has_scope(auth.scopes, "admin:*")
    )
