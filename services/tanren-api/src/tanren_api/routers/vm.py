"""VM management endpoints."""

from __future__ import annotations

from typing import Annotated

from fastapi import APIRouter, Depends, Path

from tanren_api.dependencies import get_event_store, get_job_queue, get_state_store
from tanren_api.models import (
    ProvisionRequest,
    VMProvisionAccepted,
    VMProvisionStatus,
    VMReleaseConfirmed,
    VMSummary,
)
from tanren_api.services.vm import VMService
from tanren_core.store.protocols import EventStore, JobQueue, StateStore

router = APIRouter(tags=["vm"])


def _vm_service(event_store: EventStore, job_queue: JobQueue, state_store: StateStore) -> VMService:
    return VMService(event_store=event_store, job_queue=job_queue, state_store=state_store)


@router.get("/vm")
async def list_vms(
    event_store: Annotated[EventStore, Depends(get_event_store)],
    job_queue: Annotated[JobQueue, Depends(get_job_queue)],
    state_store: Annotated[StateStore, Depends(get_state_store)],
) -> list[VMSummary]:
    """List active VM assignments."""
    return await _vm_service(event_store, job_queue, state_store).list_vms()


@router.post("/vm/provision")
async def provision_vm(
    body: ProvisionRequest,
    event_store: Annotated[EventStore, Depends(get_event_store)],
    job_queue: Annotated[JobQueue, Depends(get_job_queue)],
    state_store: Annotated[StateStore, Depends(get_state_store)],
) -> VMProvisionAccepted:
    """Provision a new VM (non-blocking)."""
    return await _vm_service(event_store, job_queue, state_store).provision(body)


@router.get("/vm/provision/{env_id}")
async def get_provision_status(
    env_id: Annotated[str, Path(description="Provisioning tracking identifier")],
    event_store: Annotated[EventStore, Depends(get_event_store)],
    job_queue: Annotated[JobQueue, Depends(get_job_queue)],
    state_store: Annotated[StateStore, Depends(get_state_store)],
) -> VMProvisionStatus:
    """Poll status of an in-progress or completed VM provisioning."""
    return await _vm_service(event_store, job_queue, state_store).get_provision_status(env_id)


@router.delete("/vm/{vm_id}")
async def release_vm(
    vm_id: Annotated[str, Path(description="VM identifier")],
    event_store: Annotated[EventStore, Depends(get_event_store)],
    job_queue: Annotated[JobQueue, Depends(get_job_queue)],
    state_store: Annotated[StateStore, Depends(get_state_store)],
) -> VMReleaseConfirmed:
    """Release a VM assignment."""
    return await _vm_service(event_store, job_queue, state_store).release(vm_id)


@router.post("/vm/dry-run")
async def dry_run_provision(
    body: ProvisionRequest,
    event_store: Annotated[EventStore, Depends(get_event_store)],
    job_queue: Annotated[JobQueue, Depends(get_job_queue)],
    state_store: Annotated[StateStore, Depends(get_state_store)],
) -> VMProvisionAccepted:
    """Dry-run provision — enqueue a DRY_RUN step and return dispatch_id for polling."""
    return await _vm_service(event_store, job_queue, state_store).dry_run(body)
