"""Run lifecycle endpoints — provision, execute, teardown, full."""

from __future__ import annotations

from typing import Annotated

from fastapi import APIRouter, Depends, Path

from tanren_api.dependencies import get_api_store, get_config, get_execution_env
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
from tanren_api.state import APIStateStore
from tanren_core.adapters.protocols import ExecutionEnvironment
from tanren_core.config import Config

router = APIRouter(tags=["run"])


def _run_service(
    store: APIStateStore,
    config: Config | None = None,
    execution_env: ExecutionEnvironment | None = None,
) -> RunService:
    """Build a RunService with the given dependencies.

    Returns:
        RunService: Configured service instance.
    """
    return RunService(store, config, execution_env)


@router.post("/run/provision")
async def run_provision(
    body: ProvisionRequest,
    store: Annotated[APIStateStore, Depends(get_api_store)],
    execution_env: Annotated[ExecutionEnvironment | None, Depends(get_execution_env)],
    config: Annotated[Config, Depends(get_config)],
) -> RunEnvironment:
    """Provision a remote execution environment (non-blocking).

    Returns:
        RunEnvironment: Provisioning environment with tracking env_id.
    """
    return await _run_service(store, config, execution_env).provision(body)


@router.post("/run/{env_id}/execute")
async def run_execute(
    env_id: Annotated[str, Path(description="Environment identifier")],
    body: ExecuteRequest,
    store: Annotated[APIStateStore, Depends(get_api_store)],
    execution_env: Annotated[ExecutionEnvironment | None, Depends(get_execution_env)],
    config: Annotated[Config, Depends(get_config)],
) -> RunExecuteAccepted:
    """Execute a phase against a provisioned environment.

    Returns:
        RunExecuteAccepted: Accepted response with env_id and dispatch_id.
    """
    return await _run_service(store, config, execution_env).execute(env_id, body)


@router.post("/run/{env_id}/teardown")
async def run_teardown(
    env_id: Annotated[str, Path(description="Environment identifier")],
    store: Annotated[APIStateStore, Depends(get_api_store)],
    execution_env: Annotated[ExecutionEnvironment | None, Depends(get_execution_env)],
) -> RunTeardownAccepted:
    """Teardown a provisioned environment.

    Returns:
        RunTeardownAccepted: Confirmation that teardown has been initiated.
    """
    return await _run_service(store, execution_env=execution_env).teardown(env_id)


@router.post("/run/full")
async def run_full(
    body: RunFullRequest,
    store: Annotated[APIStateStore, Depends(get_api_store)],
    execution_env: Annotated[ExecutionEnvironment | None, Depends(get_execution_env)],
    config: Annotated[Config, Depends(get_config)],
) -> DispatchAccepted:
    """Full lifecycle: provision, execute, teardown. Returns ID for polling.

    Returns:
        DispatchAccepted: Accepted response with dispatch_id for polling.
    """
    return await _run_service(store, config, execution_env).full(body)


@router.get("/run/{env_id}/status")
async def run_status(
    env_id: Annotated[str, Path(description="Environment identifier")],
    store: Annotated[APIStateStore, Depends(get_api_store)],
) -> RunStatus:
    """Poll status of a running environment.

    Returns:
        RunStatus: Current status of the environment.
    """
    return await _run_service(store).status(env_id)
