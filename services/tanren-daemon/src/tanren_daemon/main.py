"""Tanren daemon entry point — runs the queue-consuming worker."""

from __future__ import annotations

import asyncio
import logging
from typing import TYPE_CHECKING

from tanren_core.adapters.dotenv_validator import DotenvEnvValidator
from tanren_core.adapters.git_postflight import GitPostflightRunner
from tanren_core.adapters.git_preflight import GitPreflightRunner
from tanren_core.adapters.local_environment import LocalExecutionEnvironment
from tanren_core.adapters.subprocess_spawner import SubprocessSpawner
from tanren_core.config import load_config_env
from tanren_core.store.factory import create_store
from tanren_core.worker import Worker
from tanren_core.worker_config import WorkerConfig

if TYPE_CHECKING:
    import asyncpg

    from tanren_core.adapters.protocols import ExecutionEnvironment, VMStateStore

logger = logging.getLogger(__name__)


def _build_execution_env(
    config: WorkerConfig,
    pool: asyncpg.Pool | None = None,
) -> tuple[ExecutionEnvironment, VMStateStore | None]:
    """Build the appropriate ExecutionEnvironment based on config.

    Args:
        config: Worker configuration.
        pool: Optional asyncpg pool for Postgres-backed VM state.

    Returns:
        Tuple of (ExecutionEnvironment, VMStateStore | None).
        SSH environment when remote_config_path is set, otherwise
        a local environment for local-only deployments.
    """
    if config.remote_config_path:
        from tanren_core.builder import build_ssh_execution_environment  # noqa: PLC0415

        return build_ssh_execution_environment(config, pool=pool)

    logger.info("No WM_REMOTE_CONFIG set — using local execution environment")
    env = LocalExecutionEnvironment(
        env_validator=DotenvEnvValidator(),
        preflight=GitPreflightRunner(),
        postflight=GitPostflightRunner(),
        spawner=SubprocessSpawner(),
        config=config,
    )
    return env, None


async def _run() -> None:
    load_config_env()
    config = WorkerConfig.from_env()
    db_url = config.db_url or "tanren.db"
    store = await create_store(db_url)

    # Pass the Postgres pool to the builder so VM state uses the same
    # backend as the event store (avoids SQLite/Postgres state split)
    pg_pool: asyncpg.Pool | None = getattr(store, "_pool", None)
    execution_env, vm_store = _build_execution_env(config, pool=pg_pool)

    # Recover stale state from prior crashes before processing new work
    stale_steps = await store.recover_stale_steps()
    if stale_steps:
        logger.info("Recovered %d stale running step(s) on startup", stale_steps)

    if config.remote_config_path and hasattr(execution_env, "recover_stale_assignments"):
        recovered: int = await execution_env.recover_stale_assignments()  # type: ignore[union-attr]
        if recovered:
            logger.info("Recovered %d stale VM assignment(s) on startup", recovered)

    worker = Worker(
        config=config,
        event_store=store,
        job_queue=store,
        state_store=store,
        execution_env=execution_env,
    )
    try:
        await worker.run()
    finally:
        if vm_store is not None:
            await vm_store.close()
        await store.close()


def main() -> None:
    """Start the worker event loop."""
    asyncio.run(_run())


if __name__ == "__main__":
    main()
