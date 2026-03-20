"""Integration tests for DispatchRouter queue routing and consumers."""

import asyncio
from pathlib import Path
from unittest.mock import AsyncMock

import pytest

from tanren_core.queues import DispatchRouter
from tanren_core.schemas import Cli, Dispatch, Phase


def _make_dispatch(cli: Cli, workflow_id: str = "wf-test-1-100") -> Dispatch:
    return Dispatch(
        workflow_id=workflow_id,
        phase=Phase.DO_TASK,
        project="test",
        spec_folder="specs/test",
        branch="feature-1",
        cli=cli,
        timeout=60,
    )


@pytest.mark.asyncio
class TestDispatchRouterRouting:
    async def test_route_impl_queue(self):
        """OPENCODE and CLAUDE route to impl queue."""
        handler = AsyncMock()
        router = DispatchRouter(handler)

        d1 = _make_dispatch(Cli.OPENCODE, "wf-test-1-100")
        d2 = _make_dispatch(Cli.CLAUDE, "wf-test-2-200")
        router.route(Path("/d/1.json"), d1)
        router.route(Path("/d/2.json"), d2)

        assert router._impl_queue.qsize() == 2
        assert router._audit_queue.qsize() == 0
        assert router._gate_queue.qsize() == 0

    async def test_route_audit_queue(self):
        """CODEX routes to audit queue."""
        handler = AsyncMock()
        router = DispatchRouter(handler)

        d = _make_dispatch(Cli.CODEX)
        router.route(Path("/d/1.json"), d)

        assert router._audit_queue.qsize() == 1
        assert router._impl_queue.qsize() == 0

    async def test_route_gate_queue(self):
        """BASH routes to gate queue."""
        handler = AsyncMock()
        router = DispatchRouter(handler)

        d = _make_dispatch(Cli.BASH)
        router.route(Path("/d/1.json"), d)

        assert router._gate_queue.qsize() == 1

    async def test_get_stats_idle(self):
        """Stats show 0 active and 0 queued when idle."""
        handler = AsyncMock()
        router = DispatchRouter(handler)
        active, queued = router.get_stats()
        assert active == 0
        assert queued == 0

    async def test_get_stats_with_queued(self):
        """Stats reflect queued dispatches."""
        handler = AsyncMock()
        router = DispatchRouter(handler)

        router.route(Path("/d/1.json"), _make_dispatch(Cli.OPENCODE))
        router.route(Path("/d/2.json"), _make_dispatch(Cli.CODEX))
        router.route(Path("/d/3.json"), _make_dispatch(Cli.BASH))

        _, queued = router.get_stats()
        assert queued == 3


@pytest.mark.asyncio
class TestDispatchRouterConsumers:
    async def test_consumer_processes_dispatch(self):
        """Consumer calls handler and drains queue."""
        handler = AsyncMock()
        router = DispatchRouter(handler)

        d = _make_dispatch(Cli.OPENCODE)
        router.route(Path("/d/1.json"), d)

        router.start_consumers()
        # Give consumer time to process
        await asyncio.sleep(0.05)
        await router.stop()

        handler.assert_awaited_once()
        args = handler.call_args[0]
        assert args[0] == Path("/d/1.json")
        assert args[1].cli == Cli.OPENCODE

    async def test_consumer_continues_after_handler_error(self):
        """Consumer should not die when handler raises."""
        call_count = 0

        async def failing_then_ok(path: Path, dispatch: Dispatch) -> None:  # noqa: RUF029 — async required by interface
            nonlocal call_count
            call_count += 1
            if call_count == 1:
                raise RuntimeError("boom")

        router = DispatchRouter(failing_then_ok)

        router.route(Path("/d/1.json"), _make_dispatch(Cli.OPENCODE, "wf-test-1-100"))
        router.route(Path("/d/2.json"), _make_dispatch(Cli.OPENCODE, "wf-test-2-200"))

        router.start_consumers()
        await asyncio.sleep(0.1)
        await router.stop()

        assert call_count == 2

    async def test_gate_parallel_consumer(self):
        """Gate consumer allows concurrent dispatches up to semaphore limit."""
        active_count = 0
        max_active = 0

        async def slow_handler(path: Path, dispatch: Dispatch) -> None:
            nonlocal active_count, max_active
            active_count += 1
            max_active = max(max_active, active_count)
            await asyncio.sleep(0.05)
            active_count -= 1

        router = DispatchRouter(slow_handler, max_gate=3)

        for i in range(3):
            router.route(Path(f"/d/{i}.json"), _make_dispatch(Cli.BASH, f"wf-test-{i}-100"))

        router.start_consumers()
        await asyncio.sleep(0.15)
        await router.stop()

        assert max_active >= 2  # At least some parallelism

    async def test_stop_cancels_all_tasks(self):
        """stop() cancels all consumer tasks."""
        handler = AsyncMock()
        router = DispatchRouter(handler)
        tasks = router.start_consumers()
        assert len(tasks) == 3

        await router.stop()
        assert all(t.done() for t in tasks)
        assert len(router._tasks) == 0
