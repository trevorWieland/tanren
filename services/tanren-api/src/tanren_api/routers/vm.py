"""VM management endpoints."""

from __future__ import annotations

from typing import Annotated

from fastapi import APIRouter, Depends, Path

from tanren_api.dependencies import get_api_store, get_config, get_execution_env, get_vm_state_store
from tanren_api.models import (
    ProvisionRequest,
    VMDryRunResult,
    VMProvisionAccepted,
    VMProvisionStatus,
    VMReleaseConfirmed,
    VMSummary,
)
from tanren_api.services import VMService
from tanren_api.state import APIStateStore
from tanren_core.adapters.protocols import ExecutionEnvironment, VMStateStore
from tanren_core.config import Config

router = APIRouter(tags=["vm"])


def _vm_service(
    store: APIStateStore,
    config: Config,
    execution_env: ExecutionEnvironment | None = None,
    vm_state_store: VMStateStore | None = None,
) -> VMService:
    """Build a VMService with the given dependencies.

    Returns:
        VMService: Configured service instance.
    """
    return VMService(store, config, execution_env, vm_state_store)


@router.get("/vm")
async def list_vms(
    vm_state_store: Annotated[VMStateStore | None, Depends(get_vm_state_store)],
    config: Annotated[Config, Depends(get_config)],
    store: Annotated[APIStateStore, Depends(get_api_store)],
) -> list[VMSummary]:
    """List active VM assignments.

    Returns:
        list[VMSummary]: Active VM assignments.
    """
    return await _vm_service(store, config, vm_state_store=vm_state_store).list_vms()


@router.post("/vm/provision")
async def provision_vm(
    body: ProvisionRequest,
    store: Annotated[APIStateStore, Depends(get_api_store)],
    execution_env: Annotated[ExecutionEnvironment | None, Depends(get_execution_env)],
    config: Annotated[Config, Depends(get_config)],
) -> VMProvisionAccepted:
    """Provision a new VM (non-blocking).

    Returns:
        VMProvisionAccepted: Accepted response with tracking env_id.
    """
    return await _vm_service(store, config, execution_env).provision(body)


@router.get("/vm/provision/{env_id}")
async def get_provision_status(
    env_id: Annotated[str, Path(description="Provisioning tracking identifier")],
    store: Annotated[APIStateStore, Depends(get_api_store)],
    config: Annotated[Config, Depends(get_config)],
) -> VMProvisionStatus:
    """Poll status of an in-progress or completed VM provisioning.

    Returns:
        VMProvisionStatus: Current provisioning status.
    """
    return await _vm_service(store, config).get_provision_status(env_id)


@router.delete("/vm/{vm_id}")
async def release_vm(
    vm_id: Annotated[str, Path(description="VM identifier")],
    vm_state_store: Annotated[VMStateStore | None, Depends(get_vm_state_store)],
    execution_env: Annotated[ExecutionEnvironment | None, Depends(get_execution_env)],
    config: Annotated[Config, Depends(get_config)],
    store: Annotated[APIStateStore, Depends(get_api_store)],
) -> VMReleaseConfirmed:
    """Release a VM assignment.

    Returns:
        VMReleaseConfirmed: Confirmation of the released VM.
    """
    return await _vm_service(store, config, execution_env, vm_state_store).release(vm_id)


@router.post("/vm/dry-run")
async def dry_run_provision(
    body: ProvisionRequest,
    execution_env: Annotated[ExecutionEnvironment | None, Depends(get_execution_env)],
    config: Annotated[Config, Depends(get_config)],
    store: Annotated[APIStateStore, Depends(get_api_store)],
) -> VMDryRunResult:
    """Dry-run provision — show what would happen without creating resources.

    Returns:
        VMDryRunResult: Simulated provisioning result.
    """
    return await _vm_service(store, config, execution_env).dry_run(body)
