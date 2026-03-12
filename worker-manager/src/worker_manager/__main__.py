import asyncio

from worker_manager.config import load_config_env
from worker_manager.manager import WorkerManager


def main() -> None:
    load_config_env()
    manager = WorkerManager()
    asyncio.run(manager.run())


if __name__ == "__main__":
    main()
