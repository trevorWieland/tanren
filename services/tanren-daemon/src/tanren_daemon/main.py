"""Tanren daemon entry point — runs the worker manager."""

import asyncio

from tanren_core.config import load_config_env
from tanren_core.manager import WorkerManager


def main() -> None:
    """Load config and start the worker manager event loop."""
    load_config_env()
    manager = WorkerManager()
    asyncio.run(manager.run())


if __name__ == "__main__":
    main()
