"""VM management endpoints."""
# ruff: noqa: DOC201

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
from tanren_core.adapters.protocols import ExecutionEnvironment
from tanren_core.adapters.sqlite_vm_state import SqliteVMStateStore
from tanren_core.config import Config

router = APIRouter(tags=["vm"])


def _vm_service(
    store: APIStateStore,
    config: Config,
    execution_env: ExecutionEnvironment | None = None,
    vm_state_store: SqliteVMStateStore | None = None,
) -> VMService:
    return VMService(store, config, execution_env, vm_state_store)


@router.get("/vm")
async def list_vms(
    vm_state_store: Annotated[SqliteVMStateStore | None, Depends(get_vm_state_store)],
    config: Annotated[Config, Depends(get_config)],
    store: Annotated[APIStateStore, Depends(get_api_store)],
) -> list[VMSummary]:
    """List active VM assignments."""
    return await _vm_service(store, config, vm_state_store=vm_state_store).list_vms()


@router.post("/vm/provision")
async def provision_vm(
    body: ProvisionRequest,
    store: Annotated[APIStateStore, Depends(get_api_store)],
    execution_env: Annotated[ExecutionEnvironment | None, Depends(get_execution_env)],
    config: Annotated[Config, Depends(get_config)],
) -> VMProvisionAccepted:
    """Provision a new VM (non-blocking)."""
    return await _vm_service(store, config, execution_env).provision(body)


@router.get("/vm/provision/{env_id}")
async def get_provision_status(
    env_id: Annotated[str, Path(description="Provisioning tracking identifier")],
    store: Annotated[APIStateStore, Depends(get_api_store)],
    config: Annotated[Config, Depends(get_config)],
) -> VMProvisionStatus:
    """Poll status of an in-progress or completed VM provisioning."""
    return await _vm_service(store, config).get_provision_status(env_id)


@router.delete("/vm/{vm_id}")
async def release_vm(
    vm_id: Annotated[str, Path(description="VM identifier")],
    vm_state_store: Annotated[SqliteVMStateStore | None, Depends(get_vm_state_store)],
    execution_env: Annotated[ExecutionEnvironment | None, Depends(get_execution_env)],
    config: Annotated[Config, Depends(get_config)],
    store: Annotated[APIStateStore, Depends(get_api_store)],
) -> VMReleaseConfirmed:
    """Release a VM assignment."""
    return await _vm_service(store, config, execution_env, vm_state_store).release(vm_id)


@router.post("/vm/dry-run")
async def dry_run_provision(
    body: ProvisionRequest,
    execution_env: Annotated[ExecutionEnvironment | None, Depends(get_execution_env)],
    config: Annotated[Config, Depends(get_config)],
    store: Annotated[APIStateStore, Depends(get_api_store)],
) -> VMDryRunResult:
    """Dry-run provision — show what would happen without creating resources."""
    return await _vm_service(store, config, execution_env).dry_run(body)
