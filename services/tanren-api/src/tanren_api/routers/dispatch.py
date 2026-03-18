"""Dispatch endpoints — accept, query, and cancel dispatch requests."""
# ruff: noqa: DOC201

from __future__ import annotations

from typing import Annotated

from fastapi import APIRouter, Depends
from fastapi import Path as PathParam

from tanren_api.dependencies import get_api_store, get_config, get_emitter, get_execution_env
from tanren_api.models import (
    DispatchAccepted,
    DispatchCancelled,
    DispatchDetail,
    DispatchRequest,
)
from tanren_api.services import DispatchService
from tanren_api.state import APIStateStore
from tanren_core.adapters.protocols import EventEmitter, ExecutionEnvironment
from tanren_core.config import Config

router = APIRouter(tags=["dispatch"])


@router.post("/dispatch")
async def create_dispatch(
    body: DispatchRequest,
    store: Annotated[APIStateStore, Depends(get_api_store)],
    config: Annotated[Config, Depends(get_config)],
    emitter: Annotated[EventEmitter, Depends(get_emitter)],
    execution_env: Annotated[ExecutionEnvironment | None, Depends(get_execution_env)],
) -> DispatchAccepted:
    """Accept a new dispatch request."""
    return await DispatchService(store, config, emitter, execution_env).create(body)


@router.get("/dispatch/{dispatch_id}")
async def get_dispatch(
    dispatch_id: Annotated[str, PathParam(description="Workflow ID")],
    store: Annotated[APIStateStore, Depends(get_api_store)],
    config: Annotated[Config, Depends(get_config)],
) -> DispatchDetail:
    """Query dispatch status by workflow ID."""
    return await DispatchService(store, config).get(dispatch_id)


@router.delete("/dispatch/{dispatch_id}")
async def cancel_dispatch(
    dispatch_id: Annotated[str, PathParam(description="Workflow ID")],
    store: Annotated[APIStateStore, Depends(get_api_store)],
) -> DispatchCancelled:
    """Cancel a pending dispatch."""
    return await DispatchService(store).cancel(dispatch_id)
