"""FastAPI dependency injection helpers."""

from __future__ import annotations

from fastapi import Request

from tanren_api.settings import APISettings
from tanren_core.store.auth_protocols import AuthStore
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


def get_auth_store(request: Request) -> AuthStore:
    """Return the auth store."""
    return request.app.state.auth_store
