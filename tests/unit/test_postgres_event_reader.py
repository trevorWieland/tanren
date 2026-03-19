"""Tests for the Postgres event reader."""

from __future__ import annotations

from unittest.mock import AsyncMock, MagicMock

from tanren_core.adapters.event_reader import EventReader
from tanren_core.adapters.postgres_event_reader import PostgresEventReader


def _mock_pool():
    pool = MagicMock()
    pool.fetchval = AsyncMock(return_value=0)
    pool.fetch = AsyncMock(return_value=[])
    return pool


class TestPostgresEventReader:
    async def test_query_no_filters(self):
        pool = _mock_pool()
        pool.fetchval = AsyncMock(return_value=0)
        pool.fetch = AsyncMock(return_value=[])
        reader = PostgresEventReader(pool)

        result = await reader.query_events()

        assert result.total == 0
        assert result.events == []
        pool.fetchval.assert_awaited_once()
        pool.fetch.assert_awaited_once()

    async def test_query_with_filters(self):
        pool = _mock_pool()
        row = {
            "id": 1,
            "timestamp": "2026-01-01T00:00:00Z",
            "workflow_id": "wf-1",
            "event_type": "DispatchReceived",
            "payload": {"type": "dispatch_received", "workflow_id": "wf-1"},
        }
        pool.fetchval = AsyncMock(return_value=1)
        pool.fetch = AsyncMock(return_value=[row])
        reader = PostgresEventReader(pool)

        result = await reader.query_events(workflow_id="wf-1", event_type="DispatchReceived")

        assert result.total == 1
        assert len(result.events) == 1
        assert result.events[0].workflow_id == "wf-1"

        # Verify WHERE clause includes both filters
        fetch_sql = pool.fetch.call_args[0][0]
        assert "workflow_id = $1" in fetch_sql
        assert "event_type = $2" in fetch_sql

    async def test_query_pagination(self):
        pool = _mock_pool()
        pool.fetchval = AsyncMock(return_value=100)
        pool.fetch = AsyncMock(return_value=[])
        reader = PostgresEventReader(pool)

        result = await reader.query_events(limit=10, offset=20)

        assert result.total == 100
        # Verify LIMIT and OFFSET params
        fetch_args = pool.fetch.call_args[0]
        assert 10 in fetch_args  # limit
        assert 20 in fetch_args  # offset

    async def test_query_decodes_string_payload(self):
        """Regression: string payloads must be decoded, not skipped.

        asyncpg may return JSONB values as strings depending on codec config.
        The reader must json.loads() them instead of treating them as invalid.
        """
        pool = _mock_pool()
        row = {
            "id": 1,
            "timestamp": "2026-01-01T00:00:00Z",
            "workflow_id": "wf-1",
            "event_type": "DispatchReceived",
            "payload": '{"type": "dispatch_received", "workflow_id": "wf-1"}',
        }
        pool.fetchval = AsyncMock(return_value=1)
        pool.fetch = AsyncMock(return_value=[row])
        reader = PostgresEventReader(pool)

        result = await reader.query_events()

        assert len(result.events) == 1
        assert result.skipped == 0
        assert result.events[0].payload["type"] == "dispatch_received"

    async def test_query_skips_malformed_string_payload(self):
        pool = _mock_pool()
        row = {
            "id": 1,
            "timestamp": "2026-01-01T00:00:00Z",
            "workflow_id": "wf-1",
            "event_type": "Bad",
            "payload": "not valid json {{",
        }
        pool.fetchval = AsyncMock(return_value=1)
        pool.fetch = AsyncMock(return_value=[row])
        reader = PostgresEventReader(pool)

        result = await reader.query_events()

        assert len(result.events) == 0
        assert result.skipped == 1

    async def test_query_skips_non_dict_payload(self):
        pool = _mock_pool()
        row_good = {
            "id": 1,
            "timestamp": "2026-01-01T00:00:00Z",
            "workflow_id": "wf-1",
            "event_type": "DispatchReceived",
            "payload": {"type": "dispatch_received"},
        }
        row_bad = {
            "id": 2,
            "timestamp": "2026-01-01T00:00:01Z",
            "workflow_id": "wf-1",
            "event_type": "Bad",
            "payload": 42,
        }
        pool.fetchval = AsyncMock(return_value=2)
        pool.fetch = AsyncMock(return_value=[row_good, row_bad])
        reader = PostgresEventReader(pool)

        result = await reader.query_events()

        assert len(result.events) == 1
        assert result.skipped == 1

    def test_protocol_conformance(self):
        pool = _mock_pool()
        reader = PostgresEventReader(pool)
        assert isinstance(reader, EventReader)
