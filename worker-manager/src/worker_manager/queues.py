"""Dispatch routing: 3 role-based lanes + semaphore-gated consumers per PROTOCOL.md Section 7."""

import asyncio
import logging
from collections.abc import Awaitable, Callable
from pathlib import Path

from worker_manager.schemas import Cli, Dispatch

logger = logging.getLogger(__name__)

# Type alias for dispatch handler
DispatchHandler = Callable[[Path, Dispatch], Awaitable[None]]

# Maps CLI type to role-based queue lane
_CLI_QUEUE_MAP: dict[Cli, str] = {
    Cli.OPENCODE: "impl",
    Cli.CLAUDE: "impl",
    Cli.CODEX: "audit",
    Cli.BASH: "gate",
}


class DispatchRouter:
    """Routes dispatches to three role-based queues.

    impl  -> impl_queue  (OPENCODE, CLAUDE — max 1 concurrent)
    audit -> audit_queue (CODEX — max 1 concurrent)
    gate  -> gate_queue  (BASH — max 3 concurrent)

    Serial consumers for impl/audit ensure one agent process at a time
    (shared worktree state). Parallel consumer for gates allows concurrent
    gate checks across different specs.
    """

    def __init__(
        self,
        handler: DispatchHandler,
        max_impl: int = 1,
        max_audit: int = 1,
        max_gate: int = 3,
    ) -> None:
        self._handler = handler
        self._max_impl = max_impl
        self._max_audit = max_audit
        self._max_gate = max_gate
        self._impl_queue: asyncio.Queue[tuple[Path, Dispatch]] = asyncio.Queue()
        self._audit_queue: asyncio.Queue[tuple[Path, Dispatch]] = asyncio.Queue()
        self._gate_queue: asyncio.Queue[tuple[Path, Dispatch]] = asyncio.Queue()
        self._impl_sem = asyncio.Semaphore(max_impl)
        self._audit_sem = asyncio.Semaphore(max_audit)
        self._gate_sem = asyncio.Semaphore(max_gate)
        self._tasks: list[asyncio.Task[None]] = []

    def route(self, path: Path, dispatch: Dispatch) -> None:
        """Route a dispatch to the appropriate role-based queue."""
        lane = _CLI_QUEUE_MAP.get(dispatch.cli)
        if lane == "impl":
            self._impl_queue.put_nowait((path, dispatch))
        elif lane == "audit":
            self._audit_queue.put_nowait((path, dispatch))
        elif lane == "gate":
            self._gate_queue.put_nowait((path, dispatch))

    def start_consumers(self) -> list[asyncio.Task[None]]:
        """Start the three consumer coroutines. Returns the tasks."""
        self._tasks = [
            asyncio.create_task(
                self._consume(self._impl_queue, self._impl_sem, "impl"),
                name="impl-consumer",
            ),
            asyncio.create_task(
                self._consume(self._audit_queue, self._audit_sem, "audit"),
                name="audit-consumer",
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

    def get_stats(self) -> tuple[int, int]:
        """Return (active_processes, queued_dispatches) across all lanes."""
        active = (
            (self._max_impl - self._impl_sem._value)
            + (self._max_audit - self._audit_sem._value)
            + (self._max_gate - self._gate_sem._value)
        )
        queued = self._impl_queue.qsize() + self._audit_queue.qsize() + self._gate_queue.qsize()
        return active, queued

    async def stop(self) -> None:
        """Cancel all consumer tasks."""
        for task in self._tasks:
            task.cancel()
        await asyncio.gather(*self._tasks, return_exceptions=True)
        self._tasks.clear()
