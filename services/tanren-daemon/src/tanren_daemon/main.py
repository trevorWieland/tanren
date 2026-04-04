"""Tanren daemon entry point — runs the queue-consuming worker.

Stateless: all project/profile config travels with the dispatch.
The daemon only reads operational config (db_url, concurrency, polling).
"""

from __future__ import annotations

import asyncio
import logging
from typing import TYPE_CHECKING

from tanren_core.builder import build_execution_environment
from tanren_core.config import load_config_env
from tanren_core.store.factory import create_store
from tanren_core.worker import Worker
from tanren_core.worker_config import WorkerConfig

if TYPE_CHECKING:
    from tanren_core.adapters.protocols import ExecutionEnvironment, VMStateStore
    from tanren_core.env.environment_schema import EnvironmentProfile

logger = logging.getLogger(__name__)


async def _run() -> None:
    load_config_env()
    config = WorkerConfig.from_env()
    store = await create_store(config.db_url)

    def env_factory(
        cfg: WorkerConfig,
        profile: EnvironmentProfile,
    ) -> tuple[ExecutionEnvironment, VMStateStore | None]:
        """Build execution environment from dispatch-carried profile config.

        Returns:
            Tuple of (ExecutionEnvironment, VMStateStore | None).
        """
        return build_execution_environment(cfg, profile, db_url=config.db_url)

    # Recover stale state from prior crashes before processing new work
    stale_steps = await store.recover_stale_steps()
    if stale_steps:
        logger.info("Recovered %d stale running step(s) on startup", stale_steps)

    worker = Worker(
        config=config,
        event_store=store,
        job_queue=store,
        state_store=store,
        env_factory=env_factory,
    )
    try:
        await worker.run()
    finally:
        await store.close()


def main() -> None:
    """Start the worker event loop."""
    import os  # noqa: PLC0415

    level = os.environ.get("WM_LOG_LEVEL", "INFO").upper()
    logging.basicConfig(
        level=getattr(logging, level, logging.INFO),
        format="%(asctime)s %(levelname)-8s %(name)s: %(message)s",
    )
    asyncio.run(_run())


if __name__ == "__main__":
    main()
