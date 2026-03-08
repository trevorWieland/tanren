"""Dispatch routing: 3 queues + semaphore-gated consumers per PROTOCOL.md Section 7."""

import asyncio
import logging
from collections.abc import Callable, Coroutine
from pathlib import Path
from typing import Any

from worker_manager.schemas import Cli, Dispatch

logger = logging.getLogger(__name__)

# Type alias for dispatch handler
DispatchHandler = Callable[[Path, Dispatch], Coroutine[Any, Any, None]]


class DispatchRouter:
    """Routes dispatches to three queues based on CLI type.

    opencode -> opencode_queue (max 1 concurrent)
    codex -> codex_queue (max 1 concurrent)
    bash -> gate_queue (max 3 concurrent)
    """

    def __init__(
        self,
        handler: DispatchHandler,
        max_opencode: int = 1,
        max_codex: int = 1,
        max_gate: int = 3,
    ) -> None:
        self._handler = handler
        self._opencode_queue: asyncio.Queue[tuple[Path, Dispatch]] = asyncio.Queue()
        self._codex_queue: asyncio.Queue[tuple[Path, Dispatch]] = asyncio.Queue()
        self._gate_queue: asyncio.Queue[tuple[Path, Dispatch]] = asyncio.Queue()
        self._opencode_sem = asyncio.Semaphore(max_opencode)
        self._codex_sem = asyncio.Semaphore(max_codex)
        self._gate_sem = asyncio.Semaphore(max_gate)
        self._tasks: list[asyncio.Task[None]] = []

    def route(self, path: Path, dispatch: Dispatch) -> None:
        """Route a dispatch to the appropriate queue."""
        match dispatch.cli:
            case Cli.OPENCODE:
                self._opencode_queue.put_nowait((path, dispatch))
            case Cli.CODEX:
                self._codex_queue.put_nowait((path, dispatch))
            case Cli.BASH:
                self._gate_queue.put_nowait((path, dispatch))

    def start_consumers(self) -> list[asyncio.Task[None]]:
        """Start the three consumer coroutines. Returns the tasks."""
        self._tasks = [
            asyncio.create_task(
                self._consume(self._opencode_queue, self._opencode_sem, "opencode"),
                name="opencode-consumer",
            ),
            asyncio.create_task(
                self._consume(self._codex_queue, self._codex_sem, "codex"),
                name="codex-consumer",
            ),
            asyncio.create_task(
                self._consume_parallel(self._gate_queue, self._gate_sem, "gate"),
                name="gate-consumer",
            ),
        ]
        return self._tasks

    async def _consume(
        self,
        queue: asyncio.Queue[tuple[Path, Dispatch]],
        sem: asyncio.Semaphore,
        name: str,
    ) -> None:
        """Serial consumer: process one dispatch at a time."""
        while True:
            path, dispatch = await queue.get()
            async with sem:
                try:
                    await self._handler(path, dispatch)
                except Exception:
                    logger.exception("Error handling dispatch %s in %s consumer", path, name)
                finally:
                    queue.task_done()

    async def _consume_parallel(
        self,
        queue: asyncio.Queue[tuple[Path, Dispatch]],
        sem: asyncio.Semaphore,
        name: str,
    ) -> None:
        """Parallel consumer for gates: spawns tasks up to semaphore limit."""
        while True:
            path, dispatch = await queue.get()

            async def _run(p: Path = path, d: Dispatch = dispatch) -> None:
                async with sem:
                    try:
                        await self._handler(p, d)
                    except Exception:
                        logger.exception("Error handling dispatch %s in %s consumer", p, name)
                    finally:
                        queue.task_done()

            task = asyncio.create_task(_run(), name=f"gate-{path.stem}")
            self._tasks.append(task)

    async def stop(self) -> None:
        """Cancel all consumer tasks."""
        for task in self._tasks:
            task.cancel()
        await asyncio.gather(*self._tasks, return_exceptions=True)
        self._tasks.clear()
