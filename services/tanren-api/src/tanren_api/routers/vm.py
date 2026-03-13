"""VM management endpoints."""
# ruff: noqa: DOC201,DOC501

from __future__ import annotations

import logging
from typing import Annotated

from fastapi import APIRouter, Depends, Path

from tanren_api.dependencies import get_config, get_execution_env, get_vm_state_store
from tanren_api.errors import NotFoundError, ServiceError
from tanren_api.models import (
    ProvisionRequest,
    VMDryRunResult,
    VMReleaseConfirmed,
    VMStatus,
    VMSummary,
)
from tanren_core.adapters.protocols import ExecutionEnvironment
from tanren_core.adapters.remote_types import VMProvider, VMRequirements
from tanren_core.adapters.sqlite_vm_state import SqliteVMStateStore
from tanren_core.config import Config
from tanren_core.schemas import Cli, Dispatch, Phase

logger = logging.getLogger(__name__)

router = APIRouter(tags=["vm"])


@router.get("/vm")
async def list_vms(
    vm_state_store: Annotated[SqliteVMStateStore | None, Depends(get_vm_state_store)],
) -> list[VMSummary]:
    """List active VM assignments."""
    if vm_state_store is None:
        return []

    assignments = await vm_state_store.get_active_assignments()
    return [
        VMSummary(
            vm_id=a.vm_id,
            host=a.host,
            provider=VMProvider.MANUAL,
            workflow_id=a.workflow_id,
            project=a.project,
            status=VMStatus.ACTIVE,
            created_at=a.assigned_at,
        )
        for a in assignments
    ]


@router.post("/vm/provision")
async def provision_vm(
    body: ProvisionRequest,
    execution_env: Annotated[ExecutionEnvironment | None, Depends(get_execution_env)],
    config: Annotated[Config, Depends(get_config)],
) -> dict[str, object]:
    """Provision a new VM.

    Returns:
        VMHandle-shaped dict with vm_id, host, provider, created_at.
    """
    if execution_env is None:
        raise ServiceError("Remote execution environment not configured")

    dispatch = Dispatch(
        workflow_id=f"vm-provision-{body.project}",
        project=body.project,
        phase=Phase.DO_TASK,
        branch=body.branch,
        spec_folder="",
        cli=Cli.CLAUDE,
        timeout=1800,
        environment_profile=body.environment_profile,
    )
    handle = await execution_env.provision(dispatch, config)
    runtime = handle.runtime
    vm_handle = runtime.vm_handle  # type: ignore[union-attr]
    return vm_handle.model_dump()


@router.delete("/vm/{vm_id}")
async def release_vm(
    vm_id: Annotated[str, Path(description="VM identifier")],
    vm_state_store: Annotated[SqliteVMStateStore | None, Depends(get_vm_state_store)],
) -> VMReleaseConfirmed:
    """Release a VM assignment."""
    if vm_state_store is None:
        raise NotFoundError(f"VM {vm_id} not found")

    assignment = await vm_state_store.get_assignment(vm_id)
    if assignment is None:
        raise NotFoundError(f"VM {vm_id} not found")

    await vm_state_store.record_release(vm_id)
    return VMReleaseConfirmed(vm_id=vm_id)


@router.post("/vm/dry-run")
async def dry_run_provision(body: ProvisionRequest) -> VMDryRunResult:
    """Dry-run provision — show what would happen without creating resources."""
    requirements = VMRequirements(profile=body.environment_profile)
    return VMDryRunResult(
        provider=VMProvider.MANUAL,
        would_provision=True,
        requirements=requirements,
    )
