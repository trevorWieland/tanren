"""FastAPI dependency injection helpers."""

from __future__ import annotations

from fastapi import Request

from tanren_api.errors import ServiceError
from tanren_api.settings import APISettings
from tanren_api.state import APIStateStore
from tanren_core.adapters.protocols import EventEmitter, ExecutionEnvironment, VMStateStore
from tanren_core.config import Config


def get_settings(request: Request) -> APISettings:
    """Return the API settings stored in app state."""
    return request.app.state.settings


def get_config(request: Request) -> Config:
    """Return the core Config stored in app state.

    Raises:
        ServiceError: If config is None (WM_* env vars not set).
    """
    config = request.app.state.config
    if config is None:
        raise ServiceError("Configuration unavailable — WM_* environment variables not set")
    return config


def get_emitter(request: Request) -> EventEmitter:
    """Return the event emitter stored in app state."""
    return request.app.state.emitter


def get_api_store(request: Request) -> APIStateStore:
    """Return the API state store."""
    return request.app.state.api_store


def get_execution_env(request: Request) -> ExecutionEnvironment | None:
    """Return the execution environment, or None if not configured."""
    return request.app.state.execution_env


def get_vm_state_store(request: Request) -> VMStateStore | None:
    """Return the VM state store, or None if not configured."""
    return request.app.state.vm_state_store
