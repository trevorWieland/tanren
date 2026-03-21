"""FastAPI application factory and entry point."""

from __future__ import annotations

import logging
from collections.abc import AsyncIterator, Callable, Mapping
from contextlib import AbstractAsyncContextManager, asynccontextmanager
from typing import Any, cast

import uvicorn
from fastapi import Depends, FastAPI
from fastapi.middleware.cors import CORSMiddleware
from starlette.middleware import Middleware

from tanren_api.auth import verify_api_key
from tanren_api.errors import TanrenAPIError, tanren_error_handler
from tanren_api.mcp_auth import MCPApiKeyAuth
from tanren_api.mcp_server import mcp, set_services
from tanren_api.middleware import RequestIDMiddleware, RequestLoggingMiddleware
from tanren_api.routers import config as config_router_mod
from tanren_api.routers import dispatch as dispatch_router_mod
from tanren_api.routers import events as events_router_mod
from tanren_api.routers import health as health_router_mod
from tanren_api.routers import metrics as metrics_router_mod
from tanren_api.routers import run as run_router_mod
from tanren_api.routers import vm as vm_router_mod
from tanren_api.services import (
    ConfigService,
    DispatchService,
    EventsService,
    HealthService,
    MetricsService,
    RunService,
    VMService,
)
from tanren_api.settings import APISettings
from tanren_core.store.factory import create_sqlite_store

logger = logging.getLogger(__name__)


def create_app(settings: APISettings | None = None) -> FastAPI:
    """Build and configure the FastAPI application.

    Returns:
        Configured FastAPI instance.
    """
    settings = settings or APISettings()

    @asynccontextmanager
    async def lifespan(app: FastAPI) -> AsyncIterator[Mapping[str, Any] | None]:
        app.state.settings = settings

        # Register MCP auth middleware (clear any stale instances first)
        mcp.middleware[:] = [m for m in mcp.middleware if not isinstance(m, MCPApiKeyAuth)]
        mcp.add_middleware(MCPApiKeyAuth(settings.api_key))

        # ── Store (mandatory) ──
        store = await create_sqlite_store(settings.db_url)
        app.state.event_store = store
        app.state.job_queue = store
        app.state.state_store = store

        # ── Wire services ──
        dispatch_svc = DispatchService(
            event_store=store,
            job_queue=store,
            state_store=store,
        )
        run_svc = RunService(
            event_store=store,
            job_queue=store,
            state_store=store,
        )
        vm_svc = VMService(
            event_store=store,
            job_queue=store,
            state_store=store,
        )
        events_svc = EventsService(store)
        config_svc = ConfigService(settings, store)
        metrics_svc = MetricsService(store)

        set_services(
            health=HealthService(),
            dispatch=dispatch_svc,
            vm=vm_svc,
            run=run_svc,
            config=config_svc,
            events=events_svc,
            metrics=metrics_svc,
        )

        yield

        # Shutdown
        await store.close()

    # Build middleware stack before creating app (order matters: outermost first)
    middleware_stack: list[Middleware] = [
        Middleware(RequestIDMiddleware),  # ty: ignore[invalid-argument-type]  # ASGI middleware class; ty can't resolve Starlette's overloaded Middleware constructor
        Middleware(RequestLoggingMiddleware),  # ty: ignore[invalid-argument-type]  # same as above
    ]
    if settings.cors_origins:
        middleware_stack.append(
            Middleware(
                CORSMiddleware,  # ty: ignore[invalid-argument-type]  # Starlette CORSMiddleware is a valid middleware class; ty can't resolve the overloaded factory type
                allow_origins=settings.cors_origins,
                allow_credentials=True,
                allow_methods=["*"],
                allow_headers=["*"],
            )
        )

    # Create MCP sub-application and combine lifespans
    mcp_app = mcp.http_app(path="/")

    from fastmcp.utilities.lifespan import (
        combine_lifespans,
    )

    # cast needed: @asynccontextmanager return type doesn't satisfy Lifespan generic
    combined_lifespan = cast(
        "Callable[[FastAPI], AbstractAsyncContextManager[Mapping[str, Any] | None]]",
        combine_lifespans(lifespan, mcp_app.lifespan),  # ty: ignore[invalid-argument-type]  # @asynccontextmanager return type doesn't satisfy Starlette Lifespan generic
    )

    app = FastAPI(
        title="tanren",
        description="Tanren worker-manager HTTP API",
        version="0.1.0",
        lifespan=combined_lifespan,  # ty: ignore[invalid-argument-type]  # cast-wrapped lifespan; ty can't verify the cast target
        middleware=middleware_stack,
    )

    # Mount MCP sub-application
    app.mount("/mcp", mcp_app)

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
    app.include_router(
        metrics_router_mod.router,
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
