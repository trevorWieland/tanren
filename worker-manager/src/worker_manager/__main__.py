import asyncio

from worker_manager.manager import WorkerManager


def main() -> None:
    manager = WorkerManager()
    asyncio.run(manager.run())


if __name__ == "__main__":
    main()
