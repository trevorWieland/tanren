"""VM management endpoints."""
# ruff: noqa: DOC501 — all endpoints are stubs that raise NotImplementedAPIError

from typing import Annotated

from fastapi import APIRouter, Path

from tanren_api.errors import NotImplementedAPIError
from tanren_api.models import ProvisionRequest

router = APIRouter(tags=["vm"])


@router.get("/vm")
async def list_vms() -> list[dict]:
    """List active VM assignments."""
    raise NotImplementedAPIError(detail="VM listing not yet implemented")


@router.post("/vm/provision")
async def provision_vm(body: ProvisionRequest) -> dict:
    """Provision a new VM."""
    raise NotImplementedAPIError(detail="VM provisioning not yet implemented")


@router.delete("/vm/{vm_id}")
async def release_vm(
    vm_id: Annotated[str, Path(description="VM identifier")],
) -> dict:
    """Release a VM assignment."""
    raise NotImplementedAPIError(detail="VM release not yet implemented")


@router.post("/vm/dry-run")
async def dry_run_provision(body: ProvisionRequest) -> dict:
    """Dry-run provision — show what would happen without creating resources."""
    raise NotImplementedAPIError(detail="VM dry-run not yet implemented")
