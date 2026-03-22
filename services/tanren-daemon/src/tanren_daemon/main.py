"""Tanren daemon entry point — runs the queue-consuming worker."""

import asyncio

from tanren_core.builder import build_ssh_execution_environment
from tanren_core.config import load_config_env
from tanren_core.store.factory import create_store
from tanren_core.worker import Worker
from tanren_core.worker_config import WorkerConfig


async def _run() -> None:
    load_config_env()
    config = WorkerConfig.from_env()
    db_url = config.db_url or "tanren_events.db"
    store = await create_store(db_url)
    execution_env, _vm_store = build_ssh_execution_environment(config)
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
        await store.close()


def main() -> None:
    """Start the worker event loop."""
    asyncio.run(_run())


if __name__ == "__main__":
    main()
