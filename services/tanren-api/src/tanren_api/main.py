"""FastAPI application factory and entry point."""

from __future__ import annotations

import logging
from collections.abc import AsyncIterator
from contextlib import asynccontextmanager

import uvicorn
from fastapi import Depends, FastAPI
from fastapi.middleware.cors import CORSMiddleware
from fastmcp.utilities.lifespan import combine_lifespans
from starlette.applications import Starlette

from tanren_api.auth import resolve_auth
from tanren_api.errors import TanrenAPIError, tanren_error_handler
from tanren_api.mcp_auth import MCPApiKeyAuth
from tanren_api.mcp_server import (
    mcp,
    set_auth_store,
    set_config_resolver,
    set_services,
    set_worker_config,
)
from tanren_api.middleware import RequestIDMiddleware, RequestLoggingMiddleware
from tanren_api.routers import config as config_router_mod
from tanren_api.routers import dispatch as dispatch_router_mod
from tanren_api.routers import events as events_router_mod
from tanren_api.routers import health as health_router_mod
from tanren_api.routers import keys as keys_router_mod
from tanren_api.routers import metrics as metrics_router_mod
from tanren_api.routers import run as run_router_mod
from tanren_api.routers import users as users_router_mod
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
from tanren_core.store.factory import create_store
from tanren_core.worker_config import WorkerConfig

logger = logging.getLogger(__name__)


def _add_middleware(app: FastAPI, cls: type, **kwargs: str | bool | list[str]) -> None:
    """Add ASGI middleware — thin wrapper to satisfy ty's ParamSpec limitation."""
    app.add_middleware(cls, **kwargs)


def create_app(settings: APISettings | None = None) -> FastAPI:
    """Build and configure the FastAPI application.

    Returns:
        Configured FastAPI instance.
    """
    settings = settings or APISettings()

    # Create MCP sub-application (before lifespan so it can be mounted on app)
    mcp_app = mcp.http_app(path="/")

    @asynccontextmanager
    async def app_lifespan(app: Starlette) -> AsyncIterator[None]:
        app.state.settings = settings

        # ── Store (mandatory) ──
        store = await create_store(settings.db_url)
        app.state.event_store = store
        app.state.job_queue = store
        app.state.state_store = store
        app.state.auth_store = store

        # ── Legacy admin key seeding ──
        if settings.api_key:
            from tanren_api.auth_seed import seed_legacy_admin_key

            await seed_legacy_admin_key(store, store, settings.api_key)

        # Wire auth store for MCP resource limit checks
        set_auth_store(store)

        # Register MCP auth middleware (clear any stale instances first)
        mcp.middleware[:] = [m for m in mcp.middleware if not isinstance(m, MCPApiKeyAuth)]
        mcp.add_middleware(MCPApiKeyAuth(store))

        # ── WorkerConfig (optional — needed for dispatch/resolve) ──
        wc: WorkerConfig | None = None
        try:
            wc = WorkerConfig.from_env()
            set_worker_config(wc)

            from tanren_core.config_resolver import DiskConfigResolver

            set_config_resolver(DiskConfigResolver(wc.github_dir))
            logger.info("MCP dispatch resolution configured from WM_* env vars")
        except ValueError:
            logger.warning(
                "WM_* env vars not set — dispatch/resolve operations will fail. "
                "Set WM_GITHUB_DIR, WM_REMOTE_CONFIG, etc. in the API environment."
            )

        # ── Wire services ──
        dispatch_svc = DispatchService(
            event_store=store,
            job_queue=store,
            state_store=store,
            config=wc,
        )
        run_svc = RunService(
            event_store=store,
            job_queue=store,
            state_store=store,
            config=wc,
        )
        vm_svc = VMService(
            event_store=store,
            job_queue=store,
            state_store=store,
        )
        events_svc = EventsService(store)
        config_svc = ConfigService(settings, store, worker_config=wc)
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

        try:
            yield
        finally:
            await store.close()

    app = FastAPI(
        title="tanren",
        description="Tanren HTTP API",
        version="0.1.0",
        lifespan=combine_lifespans(app_lifespan, mcp_app.lifespan),
    )

    # Middleware: outermost first (add_middleware wraps each layer around the app)
    _add_middleware(app, RequestIDMiddleware)
    _add_middleware(app, RequestLoggingMiddleware)
    if settings.cors_origins:
        _add_middleware(
            app,
            CORSMiddleware,
            allow_origins=settings.cors_origins,
            allow_credentials=True,
            allow_methods=["*"],
            allow_headers=["*"],
        )

    # Mount MCP sub-application
    app.mount("/mcp", mcp_app)

    # Routers
    app.include_router(health_router_mod.router)
    app.include_router(
        dispatch_router_mod.router,
        prefix="/api/v1",
        dependencies=[Depends(resolve_auth)],
    )
    app.include_router(
        vm_router_mod.router,
        prefix="/api/v1",
        dependencies=[Depends(resolve_auth)],
    )
    app.include_router(
        run_router_mod.router,
        prefix="/api/v1",
        dependencies=[Depends(resolve_auth)],
    )
    app.include_router(
        config_router_mod.router,
        prefix="/api/v1",
        dependencies=[Depends(resolve_auth)],
    )
    app.include_router(
        events_router_mod.router,
        prefix="/api/v1",
        dependencies=[Depends(resolve_auth)],
    )
    app.include_router(
        metrics_router_mod.router,
        prefix="/api/v1",
        dependencies=[Depends(resolve_auth)],
    )
    app.include_router(
        users_router_mod.router,
        prefix="/api/v1",
        dependencies=[Depends(resolve_auth)],
    )
    app.include_router(
        keys_router_mod.router,
        prefix="/api/v1",
        dependencies=[Depends(resolve_auth)],
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
