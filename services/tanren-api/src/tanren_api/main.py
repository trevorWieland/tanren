"""FastAPI application factory and entry point."""

from __future__ import annotations

import asyncio
import logging
from collections.abc import AsyncIterator
from contextlib import asynccontextmanager

import uvicorn
from fastapi import Depends, FastAPI
from fastapi.middleware.cors import CORSMiddleware
from starlette.middleware import Middleware

from tanren_api.auth import verify_api_key
from tanren_api.errors import TanrenAPIError, tanren_error_handler
from tanren_api.middleware import RequestIDMiddleware, RequestLoggingMiddleware
from tanren_api.routers import config as config_router_mod
from tanren_api.routers import dispatch as dispatch_router_mod
from tanren_api.routers import events as events_router_mod
from tanren_api.routers import health as health_router_mod
from tanren_api.routers import run as run_router_mod
from tanren_api.routers import vm as vm_router_mod
from tanren_api.settings import APISettings
from tanren_api.state import APIStateStore
from tanren_core.adapters.null_emitter import NullEventEmitter
from tanren_core.adapters.sqlite_emitter import SqliteEventEmitter
from tanren_core.builder import build_ssh_execution_environment
from tanren_core.config import Config, load_config_env

logger = logging.getLogger(__name__)


def create_app(settings: APISettings | None = None) -> FastAPI:
    """Build and configure the FastAPI application.

    Returns:
        Configured FastAPI instance.
    """
    settings = settings or APISettings()

    @asynccontextmanager
    async def lifespan(app: FastAPI) -> AsyncIterator[None]:
        app.state.settings = settings
        load_config_env()
        try:
            app.state.config = Config.from_env()
        except Exception:
            # Allow app to start even without WM_* config (e.g. for OpenAPI export)
            app.state.config = None
        db = settings.events_db
        if db is None and app.state.config is not None:
            db = app.state.config.events_db
        if db:
            app.state.emitter = SqliteEventEmitter(db)
        else:
            app.state.emitter = NullEventEmitter()

        # API state store and execution environment
        app.state.api_store = APIStateStore()
        app.state.execution_env = None
        app.state.vm_state_store = None

        if app.state.config and app.state.config.remote_config_path:
            try:
                env, vm_store = await asyncio.to_thread(
                    build_ssh_execution_environment, app.state.config, app.state.emitter
                )
                app.state.execution_env = env
                app.state.vm_state_store = vm_store
            except Exception:
                logger.warning("Failed to initialize remote execution environment", exc_info=True)
            else:
                if hasattr(env, "recover_stale_assignments"):
                    try:
                        recovered = await env.recover_stale_assignments()
                        if recovered:
                            logger.info("Recovered %d stale VM assignment(s) on startup", recovered)
                    except Exception:
                        logger.warning("Failed to recover stale VM assignments", exc_info=True)

        yield

        # Shutdown
        await app.state.api_store.shutdown()
        if app.state.execution_env is not None:
            await app.state.execution_env.close()
        await app.state.emitter.close()

    # Build middleware stack before creating app (order matters: outermost first)
    middleware_stack: list[Middleware] = [
        Middleware(RequestIDMiddleware),  # type: ignore[arg-type]
        Middleware(RequestLoggingMiddleware),  # type: ignore[arg-type]
    ]
    if settings.cors_origins:
        middleware_stack.append(
            Middleware(
                CORSMiddleware,  # type: ignore[arg-type]
                allow_origins=settings.cors_origins,
                allow_credentials=True,
                allow_methods=["*"],
                allow_headers=["*"],
            )
        )

    app = FastAPI(
        title="tanren",
        description="Tanren worker-manager HTTP API",
        version="0.1.0",
        lifespan=lifespan,
        middleware=middleware_stack,
    )

    # Routers
    app.include_router(health_router_mod.router)
    app.include_router(
        dispatch_router_mod.router,
        prefix="/api/v1",
        dependencies=[Depends(verify_api_key)],
    )
    app.include_router(
        vm_router_mod.router,
        prefix="/api/v1",
        dependencies=[Depends(verify_api_key)],
    )
    app.include_router(
        run_router_mod.router,
        prefix="/api/v1",
        dependencies=[Depends(verify_api_key)],
    )
    app.include_router(
        config_router_mod.router,
        prefix="/api/v1",
        dependencies=[Depends(verify_api_key)],
    )
    app.include_router(
        events_router_mod.router,
        prefix="/api/v1",
        dependencies=[Depends(verify_api_key)],
    )

    # Global exception handler
    app.add_exception_handler(TanrenAPIError, tanren_error_handler)

    return app


app = create_app()


def main() -> None:
    """Start the API server."""
    settings = APISettings()
    uvicorn.run(
        "tanren_api.main:app",
        host=settings.host,
        port=settings.port,
        workers=settings.workers,
        log_level=settings.log_level,
    )
