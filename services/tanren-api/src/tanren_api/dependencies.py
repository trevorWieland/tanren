"""FastAPI dependency injection helpers."""

from __future__ import annotations

from fastapi import Request

from tanren_api.services import DispatchService, RunService, VMService
from tanren_api.settings import APISettings
from tanren_core.store.protocols import EventStore, JobQueue, StateStore


def get_settings(request: Request) -> APISettings:
    """Return the API settings stored in app state."""
    return request.app.state.settings


def get_event_store(request: Request) -> EventStore:
    """Return the unified event store."""
    return request.app.state.event_store


def get_job_queue(request: Request) -> JobQueue:
    """Return the job queue."""
    return request.app.state.job_queue


def get_state_store(request: Request) -> StateStore:
    """Return the state store."""
    return request.app.state.state_store


def get_dispatch_service(request: Request) -> DispatchService:
    """Return the DispatchService from app state."""
    return request.app.state.dispatch_service


def get_run_service(request: Request) -> RunService:
    """Return the RunService from app state."""
    return request.app.state.run_service


def get_vm_service(request: Request) -> VMService:
    """Return the VMService from app state."""
    return request.app.state.vm_service
