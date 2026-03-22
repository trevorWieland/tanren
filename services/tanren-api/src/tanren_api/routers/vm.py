"""VM management endpoints."""

from __future__ import annotations

from typing import Annotated

from fastapi import APIRouter, Depends, Path, Request

from tanren_api.dependencies import get_event_store, get_job_queue, get_state_store
from tanren_api.models import (
    ProvisionRequest,
    VMProvisionAccepted,
    VMProvisionStatus,
    VMReleaseConfirmed,
    VMSummary,
)
from tanren_api.services import VMService

router = APIRouter(tags=["vm"])


def _vm_service(request: Request) -> VMService:
    """Build VMService from store dependencies."""
    return VMService(
        event_store=get_event_store(request),
        job_queue=get_job_queue(request),
        state_store=get_state_store(request),
    )


@router.get("/vm")
async def list_vms(
    service: Annotated[VMService, Depends(_vm_service)],
) -> list[VMSummary]:
    """List active VM assignments."""
    return await service.list_vms()


@router.post("/vm/provision")
async def provision_vm(
    body: ProvisionRequest,
    service: Annotated[VMService, Depends(_vm_service)],
) -> VMProvisionAccepted:
    """Provision a new VM (non-blocking)."""
    return await service.provision(body)


@router.get("/vm/provision/{env_id}")
async def get_provision_status(
    env_id: Annotated[str, Path(description="Provisioning tracking identifier")],
    service: Annotated[VMService, Depends(_vm_service)],
) -> VMProvisionStatus:
    """Poll status of an in-progress or completed VM provisioning."""
    return await service.get_provision_status(env_id)


@router.delete("/vm/{vm_id}")
async def release_vm(
    vm_id: Annotated[str, Path(description="VM identifier")],
    service: Annotated[VMService, Depends(_vm_service)],
) -> VMReleaseConfirmed:
    """Release a VM assignment."""
    return await service.release(vm_id)


@router.post("/vm/dry-run")
async def dry_run_provision(
    body: ProvisionRequest,
    service: Annotated[VMService, Depends(_vm_service)],
) -> VMProvisionAccepted:
    """Dry-run provision — enqueue a DRY_RUN step and return dispatch_id for polling."""
    return await service.dry_run(body)
