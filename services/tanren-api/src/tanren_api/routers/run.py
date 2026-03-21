"""Run lifecycle endpoints — provision, execute, teardown, full."""

from __future__ import annotations

from typing import Annotated

from fastapi import APIRouter, Depends, Path, Request

from tanren_api.dependencies import (
    get_api_store,
    get_config,
    get_event_store,
    get_execution_env,
    get_job_queue,
    get_state_store,
    get_vm_state_store,
)
from tanren_api.models import (
    CheckpointDetail,
    CheckpointSummary,
    DispatchAccepted,
    ExecuteRequest,
    ProvisionRequest,
    ResumeAccepted,
    RunEnvironment,
    RunExecuteAccepted,
    RunFullRequest,
    RunStatus,
    RunTeardownAccepted,
)
from tanren_api.services import RunService, RunServiceV2
from tanren_api.state import APIStateStore
from tanren_core.adapters.protocols import ExecutionEnvironment, VMStateStore
from tanren_core.config import Config

router = APIRouter(tags=["run"])


def _run_service_v1(
    store: APIStateStore,
    config: Config | None = None,
    execution_env: ExecutionEnvironment | None = None,
    vm_state_store: VMStateStore | None = None,
) -> RunService:
    """Build a V1 RunService with the given dependencies.

    Returns:
        RunService: Configured service instance.
    """
    return RunService(store, config, execution_env, vm_state_store=vm_state_store)


def _run_service(request: Request) -> RunService | RunServiceV2:
    """Build the appropriate run service based on available infrastructure.

    Returns V2 (queue-based) when the event-sourced store is available,
    otherwise falls back to V1 (in-process).
    """
    event_store = get_event_store(request)
    if event_store is not None:
        return RunServiceV2(
            event_store=event_store,
            job_queue=get_job_queue(request),  # type: ignore[arg-type]
            state_store=get_state_store(request),  # type: ignore[arg-type]
        )
    return RunService(
        store=get_api_store(request),
        config=request.app.state.config,
        execution_env=get_execution_env(request),
        vm_state_store=get_vm_state_store(request),
    )


@router.post("/run/provision")
async def run_provision(
    body: ProvisionRequest,
    service: Annotated[RunService | RunServiceV2, Depends(_run_service)],
) -> RunEnvironment:
    """Provision a remote execution environment (non-blocking).

    Returns:
        RunEnvironment: Provisioning environment with tracking env_id.
    """
    return await service.provision(body)


@router.post("/run/{env_id}/execute")
async def run_execute(
    env_id: Annotated[str, Path(description="Environment identifier")],
    body: ExecuteRequest,
    service: Annotated[RunService | RunServiceV2, Depends(_run_service)],
) -> RunExecuteAccepted:
    """Execute a phase against a provisioned environment.

    Returns:
        RunExecuteAccepted: Accepted response with env_id and dispatch_id.
    """
    return await service.execute(env_id, body)


@router.post("/run/{env_id}/teardown")
async def run_teardown(
    env_id: Annotated[str, Path(description="Environment identifier")],
    service: Annotated[RunService | RunServiceV2, Depends(_run_service)],
) -> RunTeardownAccepted:
    """Teardown a provisioned environment.

    Returns:
        RunTeardownAccepted: Confirmation that teardown has been initiated.
    """
    return await service.teardown(env_id)


@router.post("/run/full")
async def run_full(
    body: RunFullRequest,
    service: Annotated[RunService | RunServiceV2, Depends(_run_service)],
) -> DispatchAccepted:
    """Full lifecycle: provision, execute, teardown. Returns ID for polling.

    Returns:
        DispatchAccepted: Accepted response with dispatch_id for polling.
    """
    return await service.full(body)


@router.get("/run/{env_id}/status")
async def run_status(
    env_id: Annotated[str, Path(description="Environment identifier")],
    service: Annotated[RunService | RunServiceV2, Depends(_run_service)],
) -> RunStatus:
    """Poll status of a running environment.

    Returns:
        RunStatus: Current status of the environment.
    """
    return await service.status(env_id)


@router.post("/run/{workflow_id}/resume")
async def resume_dispatch(
    workflow_id: Annotated[str, Path(description="Workflow ID to resume")],
    store: Annotated[APIStateStore, Depends(get_api_store)],
    execution_env: Annotated[ExecutionEnvironment | None, Depends(get_execution_env)],
    vm_state_store: Annotated[VMStateStore | None, Depends(get_vm_state_store)],
    config: Annotated[Config, Depends(get_config)],
) -> ResumeAccepted:
    """Resume a checkpointed dispatch.

    Returns:
        ResumeAccepted confirmation.
    """
    return await _run_service_v1(store, config, execution_env, vm_state_store).resume(workflow_id)


@router.get("/run/checkpoints")
async def get_checkpoints(
    store: Annotated[APIStateStore, Depends(get_api_store)],
    config: Annotated[Config, Depends(get_config)],
) -> list[CheckpointSummary]:
    """List all active checkpoints.

    Returns:
        List of CheckpointSummary instances.
    """
    return await _run_service_v1(store, config).get_checkpoints()


@router.get("/run/{workflow_id}/checkpoint")
async def get_checkpoint(
    workflow_id: Annotated[str, Path(description="Workflow ID")],
    store: Annotated[APIStateStore, Depends(get_api_store)],
    config: Annotated[Config, Depends(get_config)],
) -> CheckpointDetail:
    """Get checkpoint details for a specific workflow.

    Returns:
        CheckpointDetail for the workflow.
    """
    return await _run_service_v1(store, config).get_checkpoint(workflow_id)
