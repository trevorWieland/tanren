"""Tests for NullEventEmitter."""

import pytest

from worker_manager.adapters.events import DispatchReceived, Event
from worker_manager.adapters.null_emitter import NullEventEmitter


class TestNullEventEmitter:
    @pytest.mark.asyncio
    async def test_emit_is_noop(self):
        emitter = NullEventEmitter()
        event = Event(timestamp="2026-01-01T00:00:00Z", workflow_id="wf-test-1-1000")
        await emitter.emit(event)  # Should not raise

    @pytest.mark.asyncio
    async def test_close_is_noop(self):
        emitter = NullEventEmitter()
        await emitter.close()  # Should not raise

    @pytest.mark.asyncio
    async def test_emit_accepts_subclasses(self):
        emitter = NullEventEmitter()
        event = DispatchReceived(
            timestamp="2026-01-01T00:00:00Z",
            workflow_id="wf-test-1-1000",
            phase="do-task",
            project="test",
            cli="opencode",
        )
        await emitter.emit(event)
