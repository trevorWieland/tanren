"""Tests for the Postgres event emitter."""

from __future__ import annotations

import json
from unittest.mock import AsyncMock, MagicMock

from tanren_core.adapters.events import DispatchReceived
from tanren_core.adapters.postgres_emitter import PostgresEventEmitter
from tanren_core.adapters.protocols import EventEmitter


class TestPostgresEventEmitter:
    async def test_emit_inserts_row(self):
        pool = MagicMock()
        pool.execute = AsyncMock()
        emitter = PostgresEventEmitter(pool)

        event = DispatchReceived(
            timestamp="2026-01-01T00:00:00Z",
            workflow_id="wf-1",
            phase="do-task",
            project="p",
            cli="claude",
        )
        await emitter.emit(event)

        pool.execute.assert_awaited_once()
        call_args = pool.execute.call_args
        sql = call_args[0][0]
        assert "INSERT INTO events" in sql
        assert "$1" in sql
        # Verify positional params
        assert call_args[0][1] == "2026-01-01T00:00:00Z"
        assert call_args[0][2] == "wf-1"
        assert call_args[0][3] == "DispatchReceived"

    async def test_emit_serializes_payload(self):
        pool = MagicMock()
        pool.execute = AsyncMock()
        emitter = PostgresEventEmitter(pool)

        event = DispatchReceived(
            timestamp="2026-01-01T00:00:00Z",
            workflow_id="wf-1",
            phase="do-task",
            project="p",
            cli="claude",
        )
        await emitter.emit(event)

        call_args = pool.execute.call_args
        payload_str = call_args[0][4]
        payload = json.loads(payload_str)
        assert payload["workflow_id"] == "wf-1"
        assert payload["type"] == "dispatch_received"

    async def test_close_is_noop(self):
        pool = MagicMock()
        pool.close = AsyncMock()
        emitter = PostgresEventEmitter(pool)

        await emitter.close()

        pool.close.assert_not_awaited()

    def test_protocol_conformance(self):
        pool = MagicMock()
        emitter = PostgresEventEmitter(pool)
        assert isinstance(emitter, EventEmitter)
