"""Tests for queues module."""

import asyncio
from pathlib import Path

import pytest

from tanren_core.env.environment_schema import EnvironmentProfile
from tanren_core.queues import DispatchRouter
from tanren_core.schemas import Cli, Dispatch, Phase

DEFAULT_PROFILE = EnvironmentProfile(name="default")


def _make_dispatch(cli: Cli, workflow_id: str = "wf-test-1-1000") -> Dispatch:
    return Dispatch(
        workflow_id=workflow_id,
        phase=Phase.DO_TASK if cli in (Cli.OPENCODE, Cli.CLAUDE) else Phase.GATE,
        project="test",
        spec_folder="tanren/specs/test",
        branch="main",
        cli=cli,
        model="glm-5" if cli != Cli.BASH else None,
        gate_cmd="make check" if cli == Cli.BASH else None,
        context=None,
        timeout=300,
        resolved_profile=DEFAULT_PROFILE,
    )


class TestDispatchRouter:
    @pytest.mark.asyncio
    async def test_routes_opencode(self):
        handled: list[str] = []

        async def handler(path: Path, dispatch: Dispatch) -> None:  # noqa: RUF029 — async required by interface
            handled.append(dispatch.cli.value)

        router = DispatchRouter(handler)
        router.start_consumers()

        dispatch = _make_dispatch(Cli.OPENCODE)
        router.route(Path("/tmp/test.json"), dispatch)

        await asyncio.sleep(0.1)
        await router.stop()
        assert "opencode" in handled

    @pytest.mark.asyncio
    async def test_routes_codex(self):
        handled: list[str] = []

        async def handler(path: Path, dispatch: Dispatch) -> None:  # noqa: RUF029 — async required by interface
            handled.append(dispatch.cli.value)

        router = DispatchRouter(handler)
        router.start_consumers()

        dispatch = _make_dispatch(Cli.CODEX)
        dispatch.phase = Phase.AUDIT_TASK
        router.route(Path("/tmp/test.json"), dispatch)

        await asyncio.sleep(0.1)
        await router.stop()
        assert "codex" in handled

    @pytest.mark.asyncio
    async def test_routes_bash(self):
        handled: list[str] = []

        async def handler(path: Path, dispatch: Dispatch) -> None:  # noqa: RUF029 — async required by interface
            handled.append(dispatch.cli.value)

        router = DispatchRouter(handler)
        router.start_consumers()

        dispatch = _make_dispatch(Cli.BASH)
        router.route(Path("/tmp/test.json"), dispatch)

        await asyncio.sleep(0.1)
        await router.stop()
        assert "bash" in handled

    @pytest.mark.asyncio
    async def test_routes_claude_to_impl(self):
        handled: list[str] = []

        async def handler(path: Path, dispatch: Dispatch) -> None:  # noqa: RUF029 — async required by interface
            handled.append(dispatch.cli.value)

        router = DispatchRouter(handler)
        router.start_consumers()

        dispatch = _make_dispatch(Cli.CLAUDE)
        router.route(Path("/tmp/test.json"), dispatch)

        await asyncio.sleep(0.1)
        await router.stop()
        assert "claude" in handled

    def test_get_stats_idle(self):
        async def handler(path: Path, dispatch: Dispatch) -> None:
            pass

        router = DispatchRouter(handler, max_impl=1, max_audit=1, max_gate=3)
        active, queued = router.get_stats()
        assert active == 0
        assert queued == 0

    @pytest.mark.asyncio
    async def test_get_stats_with_queued(self):
        gate = asyncio.Event()

        async def handler(path: Path, dispatch: Dispatch) -> None:
            await gate.wait()

        router = DispatchRouter(handler, max_impl=1, max_audit=1, max_gate=3)
        router.start_consumers()

        # Route one opencode dispatch (will be picked up and block on gate)
        router.route(Path("/tmp/test.json"), _make_dispatch(Cli.OPENCODE))
        await asyncio.sleep(0.05)

        active, queued = router.get_stats()
        assert active == 1  # opencode is acquired
        assert queued == 0

        gate.set()
        await asyncio.sleep(0.05)
        await router.stop()

    @pytest.mark.asyncio
    async def test_parallel_gates(self):
        """Gates should run in parallel up to max_gate limit."""
        running = asyncio.Event()
        count = 0

        async def handler(path: Path, dispatch: Dispatch) -> None:
            nonlocal count
            count += 1
            running.set()
            await asyncio.sleep(0.05)

        router = DispatchRouter(handler, max_gate=3)
        router.start_consumers()

        for i in range(3):
            dispatch = _make_dispatch(Cli.BASH, f"wf-test-{i}-1000")
            router.route(Path(f"/tmp/test{i}.json"), dispatch)

        await asyncio.sleep(0.2)
        await router.stop()
        assert count == 3
