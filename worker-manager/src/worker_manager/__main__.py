import asyncio

from worker_manager.manager import WorkerManager
from worker_manager.secrets import SecretLoader


def main() -> None:
    # Autoload developer secrets at service startup.
    SecretLoader()
    manager = WorkerManager()
    asyncio.run(manager.run())


if __name__ == "__main__":
    main()
