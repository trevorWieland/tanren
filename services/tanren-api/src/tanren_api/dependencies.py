"""FastAPI dependency injection helpers."""

from fastapi import Request

from tanren_api.settings import APISettings
from tanren_core.adapters.protocols import EventEmitter
from tanren_core.config import Config


def get_settings(request: Request) -> APISettings:
    """Return the API settings stored in app state."""
    return request.app.state.settings


def get_config(request: Request) -> Config:
    """Return the core Config stored in app state."""
    return request.app.state.config


def get_emitter(request: Request) -> EventEmitter:
    """Return the event emitter stored in app state."""
    return request.app.state.emitter
