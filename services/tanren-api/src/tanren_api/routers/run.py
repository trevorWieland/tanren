"""Run lifecycle endpoints — provision, execute, teardown, full."""
# ruff: noqa: DOC501 — all endpoints are stubs that raise NotImplementedAPIError

from typing import Annotated

from fastapi import APIRouter, Path

from tanren_api.errors import NotImplementedAPIError
from tanren_api.models import DispatchAccepted, ProvisionRequest, RunFullRequest

router = APIRouter(tags=["run"])


@router.post("/run/provision")
async def run_provision(body: ProvisionRequest) -> dict:
    """Provision a remote execution environment."""
    raise NotImplementedAPIError(detail="Run provisioning not yet implemented")


@router.post("/run/{env_id}/execute")
async def run_execute(
    env_id: Annotated[str, Path(description="Environment identifier")],
) -> dict:
    """Execute a phase against a provisioned environment."""
    raise NotImplementedAPIError(detail="Run execution not yet implemented")


@router.post("/run/{env_id}/teardown")
async def run_teardown(
    env_id: Annotated[str, Path(description="Environment identifier")],
) -> dict:
    """Teardown a provisioned environment."""
    raise NotImplementedAPIError(detail="Run teardown not yet implemented")


@router.post("/run/full")
async def run_full(body: RunFullRequest) -> DispatchAccepted:
    """Full lifecycle: provision, execute, teardown. Returns ID for polling."""
    raise NotImplementedAPIError(detail="Full run not yet implemented")


@router.get("/run/{env_id}/status")
async def run_status(
    env_id: Annotated[str, Path(description="Environment identifier")],
) -> dict:
    """Poll status of a running environment."""
    raise NotImplementedAPIError(detail="Run status polling not yet implemented")
