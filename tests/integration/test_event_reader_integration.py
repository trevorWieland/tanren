"""Integration tests for SqliteEventReader with real SQLite databases."""

import json
from pathlib import Path

import aiosqlite
import pytest

from tanren_core.adapters.event_reader import SqliteEventReader, query_events


async def _create_events_db(db_path: Path, events: list[tuple]) -> None:
    """Create a test events database with the given rows."""
    async with aiosqlite.connect(str(db_path)) as conn:
        await conn.execute(
            "CREATE TABLE events ("
            "  id INTEGER PRIMARY KEY,"
            "  timestamp TEXT NOT NULL,"
            "  workflow_id TEXT NOT NULL,"
            "  event_type TEXT NOT NULL,"
            "  payload TEXT"
            ")"
        )
        await conn.executemany(
            "INSERT INTO events (id, timestamp, workflow_id, event_type, payload) "
            "VALUES (?, ?, ?, ?, ?)",
            events,
        )
        await conn.commit()


@pytest.mark.asyncio
class TestSqliteEventReader:
    async def test_query_all_events(self, tmp_path: Path):
        """Query without filters returns all events."""
        db = tmp_path / "events.db"
        await _create_events_db(
            db,
            [
                (1, "2026-01-01T00:00:00", "wf-a-1-100", "DispatchReceived", '{"a": 1}'),
                (2, "2026-01-01T01:00:00", "wf-a-1-100", "PhaseCompleted", '{"b": 2}'),
            ],
        )

        reader = SqliteEventReader(db)
        result = await reader.query_events()
        assert result.total == 2
        assert len(result.events) == 2
        assert result.skipped == 0

    async def test_query_by_workflow_id(self, tmp_path: Path):
        """Filter by workflow_id returns only matching events."""
        db = tmp_path / "events.db"
        await _create_events_db(
            db,
            [
                (1, "2026-01-01T00:00:00", "wf-a-1-100", "DispatchReceived", '{"a": 1}'),
                (2, "2026-01-01T01:00:00", "wf-b-2-200", "DispatchReceived", '{"a": 2}'),
                (3, "2026-01-01T02:00:00", "wf-a-1-100", "PhaseCompleted", '{"a": 3}'),
            ],
        )

        reader = SqliteEventReader(db)
        result = await reader.query_events(workflow_id="wf-a-1-100")
        assert result.total == 2
        assert all(e.workflow_id == "wf-a-1-100" for e in result.events)

    async def test_query_by_event_type(self, tmp_path: Path):
        """Filter by event_type returns only matching events."""
        db = tmp_path / "events.db"
        await _create_events_db(
            db,
            [
                (1, "2026-01-01T00:00:00", "wf-a-1-100", "DispatchReceived", '{"a": 1}'),
                (2, "2026-01-01T01:00:00", "wf-a-1-100", "PhaseCompleted", '{"b": 2}'),
            ],
        )

        reader = SqliteEventReader(db)
        result = await reader.query_events(event_type="PhaseCompleted")
        assert result.total == 1
        assert result.events[0].event_type == "PhaseCompleted"

    async def test_empty_database(self, tmp_path: Path):
        """Empty database returns empty result."""
        db = tmp_path / "events.db"
        await _create_events_db(db, [])

        reader = SqliteEventReader(db)
        result = await reader.query_events()
        assert result.total == 0
        assert result.events == []

    async def test_nonexistent_db(self, tmp_path: Path):
        """Missing database file returns empty result (no error)."""
        reader = SqliteEventReader(tmp_path / "nonexistent.db")
        result = await reader.query_events()
        assert result.total == 0
        assert result.events == []
        assert result.skipped == 0

    async def test_pagination_limit(self, tmp_path: Path):
        """Limit restricts returned events but total reflects all matches."""
        db = tmp_path / "events.db"
        events = [
            (i, f"2026-01-01T{i:02d}:00:00", "wf-a-1-100", "E", json.dumps({"i": i}))
            for i in range(1, 11)
        ]
        await _create_events_db(db, events)

        reader = SqliteEventReader(db)
        result = await reader.query_events(limit=3)
        assert result.total == 10
        assert len(result.events) == 3

    async def test_pagination_offset(self, tmp_path: Path):
        """Offset skips the first N events."""
        db = tmp_path / "events.db"
        events = [
            (i, f"2026-01-01T{i:02d}:00:00", "wf-a-1-100", "E", json.dumps({"i": i}))
            for i in range(1, 6)
        ]
        await _create_events_db(db, events)

        reader = SqliteEventReader(db)
        result = await reader.query_events(limit=2, offset=2)
        assert result.total == 5
        assert len(result.events) == 2

    async def test_order_by_timestamp_desc(self, tmp_path: Path):
        """Events are returned in descending timestamp order."""
        db = tmp_path / "events.db"
        await _create_events_db(
            db,
            [
                (1, "2026-01-01T01:00:00", "wf-a-1-100", "E", '{"a": 1}'),
                (2, "2026-01-01T03:00:00", "wf-a-1-100", "E", '{"a": 2}'),
                (3, "2026-01-01T02:00:00", "wf-a-1-100", "E", '{"a": 3}'),
            ],
        )

        reader = SqliteEventReader(db)
        result = await reader.query_events()
        timestamps = [e.timestamp for e in result.events]
        assert timestamps == sorted(timestamps, reverse=True)

    async def test_event_payload_parsed(self, tmp_path: Path):
        """Payload JSON is parsed into a dict."""
        db = tmp_path / "events.db"
        await _create_events_db(
            db,
            [(1, "2026-01-01T00:00:00", "wf-a-1-100", "E", '{"key": "value", "n": 42}')],
        )

        reader = SqliteEventReader(db)
        result = await reader.query_events()
        assert result.events[0].payload == {"key": "value", "n": 42}

    async def test_backward_compat_wrapper(self, tmp_path: Path):
        """Module-level query_events() delegates to SqliteEventReader."""
        db = tmp_path / "events.db"
        await _create_events_db(
            db,
            [(1, "2026-01-01T00:00:00", "wf-a-1-100", "E", '{"a": 1}')],
        )

        result = await query_events(db, workflow_id="wf-a-1-100")
        assert result.total == 1
        assert result.events[0].workflow_id == "wf-a-1-100"
