"""Run lifecycle endpoints — provision, execute, teardown, full."""

from __future__ import annotations

from typing import Annotated

from fastapi import APIRouter, Depends, Path

from tanren_api.dependencies import get_run_service
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
from tanren_api.services import RunService

router = APIRouter(tags=["run"])


@router.post("/run/provision")
async def run_provision(
    body: ProvisionRequest,
    service: Annotated[RunService, Depends(get_run_service)],
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
    service: Annotated[RunService, Depends(get_run_service)],
) -> RunExecuteAccepted:
    """Execute a phase against a provisioned environment.

    Returns:
        RunExecuteAccepted: Accepted response with env_id and dispatch_id.
    """
    return await service.execute(env_id, body)


@router.post("/run/{env_id}/teardown")
async def run_teardown(
    env_id: Annotated[str, Path(description="Environment identifier")],
    service: Annotated[RunService, Depends(get_run_service)],
) -> RunTeardownAccepted:
    """Teardown a provisioned environment.

    Returns:
        RunTeardownAccepted: Confirmation that teardown has been initiated.
    """
    return await service.teardown(env_id)


@router.post("/run/full")
async def run_full(
    body: RunFullRequest,
    service: Annotated[RunService, Depends(get_run_service)],
) -> DispatchAccepted:
    """Full lifecycle: provision, execute, teardown. Returns ID for polling.

    Returns:
        DispatchAccepted: Accepted response with dispatch_id for polling.
    """
    return await service.full(body)


@router.get("/run/{env_id}/status")
async def run_status(
    env_id: Annotated[str, Path(description="Environment identifier")],
    service: Annotated[RunService, Depends(get_run_service)],
) -> RunStatus:
    """Poll status of a running environment.

    Returns:
        RunStatus: Current status of the environment.
    """
    return await service.status(env_id)
