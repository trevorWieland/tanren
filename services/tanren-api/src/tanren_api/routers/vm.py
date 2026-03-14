"""VM management endpoints."""
# ruff: noqa: DOC201,DOC501

from __future__ import annotations

import asyncio
import contextlib
import logging
import uuid
from datetime import UTC, datetime
from typing import Annotated

from fastapi import APIRouter, Depends, Path

from tanren_api.dependencies import get_api_store, get_config, get_execution_env, get_vm_state_store
from tanren_api.errors import NotFoundError, ServiceError
from tanren_api.models import (
    ProvisionRequest,
    RunEnvironmentStatus,
    VMDryRunResult,
    VMProvisionAccepted,
    VMProvisionStatus,
    VMReleaseConfirmed,
    VMStatus,
    VMSummary,
)
from tanren_api.state import APIStateStore, EnvironmentRecord
from tanren_core.adapters.protocols import ExecutionEnvironment
from tanren_core.adapters.remote_types import VMHandle, VMProvider, VMRequirements
from tanren_core.adapters.sqlite_vm_state import SqliteVMStateStore
from tanren_core.adapters.types import EnvironmentHandle, RemoteEnvironmentRuntime
from tanren_core.config import Config
from tanren_core.remote_config import ProvisionerType, load_remote_config
from tanren_core.schemas import Cli, Dispatch, Outcome, Phase

logger = logging.getLogger(__name__)

router = APIRouter(tags=["vm"])


def _derive_provider(config: Config) -> VMProvider:
    """Derive VM provider from remote config."""
    if not config.remote_config_path:
        return VMProvider.MANUAL
    try:
        remote_cfg = load_remote_config(config.remote_config_path)
    except Exception as exc:
        logger.exception("Failed to load remote config from %s", config.remote_config_path)
        raise ServiceError("Failed to load remote config") from exc
    if remote_cfg.provisioner.type == ProvisionerType.HETZNER:
        return VMProvider.HETZNER
    return VMProvider.MANUAL


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


def _now() -> str:
    return datetime.now(UTC).isoformat()


@router.post("/vm/provision")
async def provision_vm(
    body: ProvisionRequest,
    store: Annotated[APIStateStore, Depends(get_api_store)],
    execution_env: Annotated[ExecutionEnvironment | None, Depends(get_execution_env)],
    config: Annotated[Config, Depends(get_config)],
) -> VMProvisionAccepted:
    """Provision a new VM (non-blocking).

    Returns immediately with env_id for polling via GET /vm/provision/{env_id}.
    """
    if execution_env is None:
        raise ServiceError("Remote execution environment not configured")

    env_id = str(uuid.uuid4())

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

    record = EnvironmentRecord(
        env_id=env_id,
        handle=None,
        status=RunEnvironmentStatus.PROVISIONING,
        started_at=_now(),
    )
    await store.add_environment(record)

    async def _provision_background() -> None:
        handle: EnvironmentHandle | None = None
        try:
            handle = await execution_env.provision(dispatch, config)
            runtime = handle.runtime
            if not isinstance(runtime, RemoteEnvironmentRuntime):
                raise ServiceError("Provisioned environment is not a remote runtime")
            vm_handle = runtime.vm_handle
            # Close SSH connection to prevent leak (not full teardown — that releases the VM)
            try:
                close_fn = getattr(runtime.connection, "close", None)
                if close_fn is not None:
                    await close_fn()
            except Exception:
                logger.debug(
                    "Failed to close provision-time SSH connection for %s", vm_handle.vm_id
                )
            updated = await store.try_transition_environment(
                env_id,
                from_statuses=frozenset({RunEnvironmentStatus.PROVISIONING}),
                to_status=RunEnvironmentStatus.PROVISIONED,
                handle=handle,
                vm_id=vm_handle.vm_id,
                host=vm_handle.host,
            )
            if updated is not None:
                handle = None  # Persisted — suppress finally cleanup
        except asyncio.CancelledError:
            raise
        except Exception:
            handle = None  # Error handler owns cleanup
            logger.exception("VM provision failed for %s", env_id)
            await store.try_transition_environment(
                env_id,
                from_statuses=frozenset({RunEnvironmentStatus.PROVISIONING}),
                to_status=RunEnvironmentStatus.FAILED,
                outcome=Outcome.ERROR,
                completed_at=_now(),
            )
        finally:
            if handle is not None:
                logger.warning("Cleaning up orphaned VM provision for %s", env_id)
                inner = asyncio.ensure_future(execution_env.teardown(handle))
                try:
                    await asyncio.shield(inner)
                except asyncio.CancelledError, Exception:
                    with contextlib.suppress(asyncio.CancelledError, Exception):
                        await inner

    task = asyncio.create_task(_provision_background())
    await store.update_environment(env_id, task=task)

    return VMProvisionAccepted(env_id=env_id)


@router.get("/vm/provision/{env_id}")
async def get_provision_status(
    env_id: Annotated[str, Path(description="Provisioning tracking identifier")],
    store: Annotated[APIStateStore, Depends(get_api_store)],
    config: Annotated[Config, Depends(get_config)],
) -> VMProvisionStatus:
    """Poll status of an in-progress or completed VM provisioning."""
    record = await store.get_environment(env_id)
    if record is None:
        raise NotFoundError(f"Provision {env_id} not found")

    provider = _derive_provider(config)
    if record.status == RunEnvironmentStatus.PROVISIONED:
        vm_status = VMStatus.ACTIVE
    elif record.status == RunEnvironmentStatus.FAILED:
        vm_status = VMStatus.FAILED
    else:
        vm_status = VMStatus.PROVISIONING

    return VMProvisionStatus(
        env_id=env_id,
        status=vm_status,
        vm_id=record.vm_id or None,
        host=record.host or None,
        provider=provider if record.vm_id else None,
        created_at=record.started_at,
    )


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

    if execution_env is None:
        raise ServiceError("Remote execution environment not configured")

    # Release via provider first — must succeed before updating state
    provider = _derive_provider(config)
    vm_handle = VMHandle(
        vm_id=assignment.vm_id,
        host=assignment.host,
        provider=provider,
        created_at=assignment.assigned_at,
    )
    try:
        await execution_env.release_vm(vm_handle)
    except Exception as exc:
        logger.exception("VM release failed for %s", vm_id)
        raise ServiceError("Failed to release VM") from exc

    # Update state tracking only after successful provider release
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
