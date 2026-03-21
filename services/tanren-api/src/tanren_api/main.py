"""FastAPI application factory and entry point."""

from __future__ import annotations

import asyncio
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
from tanren_api.state import APIStateStore
from tanren_core.adapters.event_reader import SqliteEventReader
from tanren_core.adapters.null_emitter import NullEventEmitter
from tanren_core.adapters.postgres_pool import is_postgres_url
from tanren_core.adapters.sqlite_emitter import SqliteEventEmitter
from tanren_core.adapters.sqlite_metrics_reader import SqliteMetricsReader
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
    async def lifespan(app: FastAPI) -> AsyncIterator[Mapping[str, Any] | None]:
        app.state.settings = settings

        # Register MCP auth middleware (clear any stale instances first)
        mcp.middleware[:] = [m for m in mcp.middleware if not isinstance(m, MCPApiKeyAuth)]
        mcp.add_middleware(MCPApiKeyAuth(settings.api_key))
        load_config_env()
        try:
            app.state.config = Config.from_env()
        except Exception:
            # Allow app to start even without WM_* config (e.g. for OpenAPI export)
            app.state.config = None
        db = settings.events_db
        if db is None and app.state.config is not None:
            db = app.state.config.events_db

        app.state.pg_pool = None
        app.state.event_reader = None
        app.state.metrics_reader = None
        if db and is_postgres_url(db):
            from tanren_core.adapters.postgres_emitter import (
                PostgresEventEmitter,
            )
            from tanren_core.adapters.postgres_event_reader import (
                PostgresEventReader,
            )
            from tanren_core.adapters.postgres_metrics_reader import (
                PostgresMetricsReader,
            )
            from tanren_core.adapters.postgres_pool import (
                create_postgres_pool,
            )

            pg_pool = await create_postgres_pool(db)
            app.state.pg_pool = pg_pool
            app.state.emitter = PostgresEventEmitter(pg_pool)
            app.state.event_reader = PostgresEventReader(pg_pool)
            app.state.metrics_reader = PostgresMetricsReader(pg_pool)
        elif db:
            app.state.emitter = SqliteEventEmitter(db)
            app.state.event_reader = SqliteEventReader(db)
            app.state.metrics_reader = SqliteMetricsReader(db)
        else:
            app.state.emitter = NullEventEmitter()

        # ── New event-sourced store (Phase 6) ──
        app.state.event_store = None
        app.state.job_queue = None
        app.state.state_store = None
        store_url = settings.db_url or db
        if store_url:
            from tanren_core.store.factory import create_sqlite_store

            try:
                store = await create_sqlite_store(store_url)
                app.state.event_store = store
                app.state.job_queue = store
                app.state.state_store = store
            except Exception:
                logger.warning("Failed to initialize event-sourced store", exc_info=True)

        # API state store and execution environment
        app.state.api_store = APIStateStore()
        app.state.execution_env = None
        app.state.vm_state_store = None

        if app.state.config and app.state.config.remote_config_path:
            try:
                env, vm_store = await asyncio.to_thread(
                    build_ssh_execution_environment,
                    app.state.config,
                    pool=app.state.pg_pool,
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

        # Wire MCP service layer — prefer V2 (queue-based) when event-sourced store is available
        config_svc = ConfigService(app.state.config) if app.state.config else None

        dispatch_svc = DispatchService(
            event_store=app.state.event_store,
            job_queue=app.state.job_queue,
            state_store=app.state.state_store,
        )
        run_svc = RunService(
            event_store=app.state.event_store,
            job_queue=app.state.job_queue,
            state_store=app.state.state_store,
        )

        set_services(
            health=HealthService(),
            dispatch=dispatch_svc,
            vm=VMService(
                store=app.state.api_store,
                config=app.state.config,
                execution_env=app.state.execution_env,
                vm_state_store=app.state.vm_state_store,
            ),
            run=run_svc,
            config=config_svc,
            events=EventsService(settings, app.state.config, event_reader=app.state.event_reader),
            metrics=MetricsService(metrics_reader=app.state.metrics_reader),
        )

        yield

        # Shutdown
        await app.state.api_store.shutdown()
        if app.state.execution_env is not None:
            await app.state.execution_env.close()
        await app.state.emitter.close()
        if app.state.event_store is not None:
            await app.state.event_store.close()
        if app.state.pg_pool is not None:
            await app.state.pg_pool.close()

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
