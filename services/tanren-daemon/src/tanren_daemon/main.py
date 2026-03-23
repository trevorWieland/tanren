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
    from tanren_core.adapters.protocols import ExecutionEnvironment, VMStateStore

logger = logging.getLogger(__name__)


def _build_execution_env(
    config: WorkerConfig,
) -> tuple[ExecutionEnvironment, VMStateStore | None]:
    """Build the appropriate ExecutionEnvironment based on config.

    Returns:
        Tuple of (ExecutionEnvironment, VMStateStore | None).
        SSH environment when remote_config_path is set, otherwise
        a local environment for local-only deployments.
    """
    if config.remote_config_path:
        from tanren_core.builder import build_ssh_execution_environment  # noqa: PLC0415

        return build_ssh_execution_environment(config)

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
    db_url = config.db_url or "tanren_events.db"
    store = await create_store(db_url)
    execution_env, _vm_store = _build_execution_env(config)
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
        if hasattr(execution_env, "close"):
            await execution_env.close()
        await store.close()


def main() -> None:
    """Start the worker event loop."""
    asyncio.run(_run())


if __name__ == "__main__":
    main()
