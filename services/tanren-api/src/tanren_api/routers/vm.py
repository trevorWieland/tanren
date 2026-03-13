"""VM management endpoints."""
# ruff: noqa: DOC201,DOC501

from __future__ import annotations

import logging
import uuid
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
from tanren_core.adapters.remote_types import VMHandle, VMProvider, VMRequirements
from tanren_core.adapters.sqlite_vm_state import SqliteVMStateStore
from tanren_core.adapters.types import RemoteEnvironmentRuntime
from tanren_core.config import Config
from tanren_core.remote_config import ProvisionerType, load_remote_config
from tanren_core.schemas import Cli, Dispatch, Phase

logger = logging.getLogger(__name__)

router = APIRouter(tags=["vm"])


def _derive_provider(config: Config) -> VMProvider:
    """Derive VM provider from remote config."""
    provider = VMProvider.MANUAL
    if config.remote_config_path:
        try:
            remote_cfg = load_remote_config(config.remote_config_path)
            if remote_cfg.provisioner.type == ProvisionerType.HETZNER:
                provider = VMProvider.HETZNER
        except Exception:
            pass
    return provider


@router.get("/vm")
async def list_vms(
    vm_state_store: Annotated[SqliteVMStateStore | None, Depends(get_vm_state_store)],
    config: Annotated[Config, Depends(get_config)],
) -> list[VMSummary]:
    """List active VM assignments."""
    if vm_state_store is None:
        return []

    provider = _derive_provider(config)
    assignments = await vm_state_store.get_active_assignments()
    return [
        VMSummary(
            vm_id=a.vm_id,
            host=a.host,
            provider=provider,
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
) -> VMHandle:
    """Provision a new VM.

    Returns:
        VMHandle with vm_id, host, provider, created_at.
    """
    if execution_env is None:
        raise ServiceError("Remote execution environment not configured")

    dispatch = Dispatch(
        workflow_id=f"vm-provision-{body.project}-{uuid.uuid4().hex[:8]}",
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
    if not isinstance(runtime, RemoteEnvironmentRuntime):
        raise ServiceError("Provisioned environment is not a remote runtime")
    vm_handle = runtime.vm_handle
    # Close the SSH connection to prevent leak (don't call full teardown — that releases the VM)
    try:
        close_fn = getattr(runtime.connection, "close", None)
        if close_fn is not None:
            await close_fn()
    except Exception:
        logger.debug("Failed to close provision-time SSH connection for %s", vm_handle.vm_id)
    return vm_handle


@router.delete("/vm/{vm_id}")
async def release_vm(
    vm_id: Annotated[str, Path(description="VM identifier")],
    vm_state_store: Annotated[SqliteVMStateStore | None, Depends(get_vm_state_store)],
    execution_env: Annotated[ExecutionEnvironment | None, Depends(get_execution_env)],
    config: Annotated[Config, Depends(get_config)],
) -> VMReleaseConfirmed:
    """Release a VM assignment."""
    if vm_state_store is None:
        raise NotFoundError(f"VM {vm_id} not found")

    assignment = await vm_state_store.get_assignment(vm_id)
    if assignment is None:
        raise NotFoundError(f"VM {vm_id} not found")

    # Release via provider first (best-effort)
    if execution_env is not None:
        provider = _derive_provider(config)
        vm_handle = VMHandle(
            vm_id=assignment.vm_id,
            host=assignment.host,
            provider=provider,
            created_at=assignment.assigned_at,
        )
        try:
            await execution_env.release_vm(vm_handle)
        except Exception:
            logger.warning("Provider release failed for VM %s", vm_id, exc_info=True)

    # Always update state tracking
    await vm_state_store.record_release(vm_id)
    return VMReleaseConfirmed(vm_id=vm_id)


@router.post("/vm/dry-run")
async def dry_run_provision(
    body: ProvisionRequest,
    execution_env: Annotated[ExecutionEnvironment | None, Depends(get_execution_env)],
    config: Annotated[Config, Depends(get_config)],
) -> VMDryRunResult:
    """Dry-run provision — show what would happen without creating resources."""
    provider = _derive_provider(config)
    requirements = VMRequirements(profile=body.environment_profile)
    return VMDryRunResult(
        provider=provider,
        would_provision=execution_env is not None,
        requirements=requirements,
    )
