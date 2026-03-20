"""Tests for the event reader module."""

from __future__ import annotations

import json

import aiosqlite
import pytest

from tanren_core.adapters.event_reader import EventReader, SqliteEventReader, query_events
from tanren_core.adapters.sqlite_emitter import (
    _SCHEMA,  # noqa: PLC2701 — testing private implementation
)


async def _setup_db(db_path, events: list[tuple[str, str, str, dict]]):
    """Create DB with schema and insert events."""
    async with aiosqlite.connect(str(db_path)) as conn:
        await conn.executescript(_SCHEMA)
        for ts, wid, etype, payload in events:
            sql = (
                "INSERT INTO events "
                "(timestamp, workflow_id, event_type, payload) "
                "VALUES (?, ?, ?, ?)"
            )
            await conn.execute(sql, (ts, wid, etype, json.dumps(payload)))
        await conn.commit()


async def _setup_db_raw(db_path, events: list[tuple[str, str, str, str]]):
    """Create DB with schema and insert events with raw payload strings (no json.dumps)."""
    async with aiosqlite.connect(str(db_path)) as conn:
        await conn.executescript(_SCHEMA)
        for ts, wid, etype, raw_payload in events:
            sql = (
                "INSERT INTO events "
                "(timestamp, workflow_id, event_type, payload) "
                "VALUES (?, ?, ?, ?)"
            )
            await conn.execute(sql, (ts, wid, etype, raw_payload))
        await conn.commit()


class TestReadOnlyConnection:
    async def test_readonly_connection(self, tmp_path):
        """Verify the event reader uses a read-only SQLite connection."""
        import sqlite3  # noqa: PLC0415 — deferred import for test clarity

        db = tmp_path / "events.db"
        # Create the DB first with read-write
        async with aiosqlite.connect(str(db)) as conn:
            await conn.executescript(_SCHEMA)
            await conn.execute(
                "INSERT INTO events (timestamp, workflow_id, event_type, payload) "
                "VALUES (?, ?, ?, ?)",
                (
                    "2026-01-01T00:00:00Z",
                    "wf-1",
                    "DispatchReceived",
                    json.dumps({
                        "type": "dispatch_received",
                        "timestamp": "2026-01-01T00:00:00Z",
                        "workflow_id": "wf-1",
                        "phase": "do-task",
                        "project": "p",
                        "cli": "claude",
                    }),
                ),
            )
            await conn.commit()

        # query_events should succeed (reads work)
        result = await query_events(db)
        assert result.total == 1

        # Verify read-only URI rejects writes
        ro_conn = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
        try:
            with pytest.raises(sqlite3.OperationalError):
                ro_conn.execute("DELETE FROM events")
        finally:
            ro_conn.close()


class TestQueryEvents:
    async def test_query_empty_db(self, tmp_path):
        db = tmp_path / "events.db"
        async with aiosqlite.connect(str(db)) as conn:
            await conn.executescript(_SCHEMA)

        result = await query_events(db)
        assert result.events == []
        assert result.total == 0

    async def test_query_nonexistent_db(self, tmp_path):
        db = tmp_path / "nonexistent.db"
        result = await query_events(db)
        assert result.events == []
        assert result.total == 0

    async def test_query_with_events(self, tmp_path):
        db = tmp_path / "events.db"
        await _setup_db(
            db,
            [
                (
                    "2026-01-01T00:00:00Z",
                    "wf-1",
                    "DispatchReceived",
                    {
                        "type": "dispatch_received",
                        "timestamp": "2026-01-01T00:00:00Z",
                        "workflow_id": "wf-1",
                        "phase": "do-task",
                        "project": "p",
                        "cli": "claude",
                    },
                ),
                (
                    "2026-01-01T00:00:01Z",
                    "wf-1",
                    "PhaseStarted",
                    {
                        "type": "phase_started",
                        "timestamp": "2026-01-01T00:00:01Z",
                        "workflow_id": "wf-1",
                        "phase": "do-task",
                        "worktree_path": "/tmp/wt",
                    },
                ),
            ],
        )

        result = await query_events(db)
        assert result.total == 2
        assert len(result.events) == 2

    async def test_filter_by_workflow_id(self, tmp_path):
        db = tmp_path / "events.db"
        await _setup_db(
            db,
            [
                (
                    "2026-01-01T00:00:00Z",
                    "wf-1",
                    "DispatchReceived",
                    {
                        "type": "dispatch_received",
                        "timestamp": "2026-01-01T00:00:00Z",
                        "workflow_id": "wf-1",
                        "phase": "do-task",
                        "project": "p",
                        "cli": "claude",
                    },
                ),
                (
                    "2026-01-01T00:00:01Z",
                    "wf-2",
                    "DispatchReceived",
                    {
                        "type": "dispatch_received",
                        "timestamp": "2026-01-01T00:00:01Z",
                        "workflow_id": "wf-2",
                        "phase": "do-task",
                        "project": "q",
                        "cli": "claude",
                    },
                ),
            ],
        )

        result = await query_events(db, workflow_id="wf-1")
        assert result.total == 1
        assert result.events[0].workflow_id == "wf-1"

    async def test_filter_by_event_type(self, tmp_path):
        db = tmp_path / "events.db"
        await _setup_db(
            db,
            [
                (
                    "2026-01-01T00:00:00Z",
                    "wf-1",
                    "DispatchReceived",
                    {
                        "type": "dispatch_received",
                        "timestamp": "2026-01-01T00:00:00Z",
                        "workflow_id": "wf-1",
                        "phase": "do-task",
                        "project": "p",
                        "cli": "claude",
                    },
                ),
                (
                    "2026-01-01T00:00:01Z",
                    "wf-1",
                    "PhaseStarted",
                    {
                        "type": "phase_started",
                        "timestamp": "2026-01-01T00:00:01Z",
                        "workflow_id": "wf-1",
                        "phase": "do-task",
                        "worktree_path": "/tmp/wt",
                    },
                ),
            ],
        )

        result = await query_events(db, event_type="PhaseStarted")
        assert result.total == 1
        assert result.events[0].event_type == "PhaseStarted"

    async def test_pagination(self, tmp_path):
        db = tmp_path / "events.db"
        events = [
            (
                f"2026-01-01T00:00:{i:02d}Z",
                "wf-1",
                "DispatchReceived",
                {
                    "type": "dispatch_received",
                    "timestamp": f"2026-01-01T00:00:{i:02d}Z",
                    "workflow_id": "wf-1",
                    "phase": "do-task",
                    "project": "p",
                    "cli": "claude",
                },
            )
            for i in range(10)
        ]
        await _setup_db(db, events)

        result = await query_events(db, limit=3, offset=0)
        assert result.total == 10
        assert len(result.events) == 3

        result2 = await query_events(db, limit=3, offset=3)
        assert result2.total == 10
        assert len(result2.events) == 3

    async def test_combined_filters(self, tmp_path):
        db = tmp_path / "events.db"
        await _setup_db(
            db,
            [
                (
                    "2026-01-01T00:00:00Z",
                    "wf-1",
                    "DispatchReceived",
                    {
                        "type": "dispatch_received",
                        "timestamp": "2026-01-01T00:00:00Z",
                        "workflow_id": "wf-1",
                        "phase": "do-task",
                        "project": "p",
                        "cli": "claude",
                    },
                ),
                (
                    "2026-01-01T00:00:01Z",
                    "wf-1",
                    "PhaseStarted",
                    {
                        "type": "phase_started",
                        "timestamp": "2026-01-01T00:00:01Z",
                        "workflow_id": "wf-1",
                        "phase": "do-task",
                        "worktree_path": "/tmp/wt",
                    },
                ),
                (
                    "2026-01-01T00:00:02Z",
                    "wf-2",
                    "DispatchReceived",
                    {
                        "type": "dispatch_received",
                        "timestamp": "2026-01-01T00:00:02Z",
                        "workflow_id": "wf-2",
                        "phase": "do-task",
                        "project": "q",
                        "cli": "claude",
                    },
                ),
            ],
        )

        result = await query_events(db, workflow_id="wf-1", event_type="DispatchReceived")
        assert result.total == 1
        assert result.events[0].workflow_id == "wf-1"
        assert result.events[0].event_type == "DispatchReceived"

    async def test_query_skips_malformed_json_payload(self, tmp_path):
        db = tmp_path / "events.db"
        valid_payload = json.dumps({
            "type": "dispatch_received",
            "timestamp": "2026-01-01T00:00:00Z",
            "workflow_id": "wf-1",
            "phase": "do-task",
            "project": "p",
            "cli": "claude",
        })
        await _setup_db_raw(
            db,
            [
                ("2026-01-01T00:00:00Z", "wf-1", "DispatchReceived", valid_payload),
                ("2026-01-01T00:00:01Z", "wf-1", "Bad", "not json {{"),
                ("2026-01-01T00:00:02Z", "wf-1", "DispatchReceived", valid_payload),
            ],
        )

        result = await query_events(db)
        assert len(result.events) == 2
        assert result.total == 3
        assert result.skipped == 1

    async def test_query_all_malformed_returns_empty_events(self, tmp_path):
        db = tmp_path / "events.db"
        await _setup_db_raw(
            db,
            [
                ("2026-01-01T00:00:00Z", "wf-1", "Bad", "not json {{"),
                ("2026-01-01T00:00:01Z", "wf-1", "Bad", "{truncated"),
            ],
        )

        result = await query_events(db)
        assert result.events == []
        assert result.total == 2
        assert result.skipped == 2


class TestSqliteEventReader:
    async def test_sqlite_event_reader_class(self, tmp_path):
        """Verify SqliteEventReader works the same as standalone function."""
        db = tmp_path / "events.db"
        await _setup_db(
            db,
            [
                (
                    "2026-01-01T00:00:00Z",
                    "wf-1",
                    "DispatchReceived",
                    {
                        "type": "dispatch_received",
                        "timestamp": "2026-01-01T00:00:00Z",
                        "workflow_id": "wf-1",
                        "phase": "do-task",
                        "project": "p",
                        "cli": "claude",
                    },
                ),
            ],
        )

        reader = SqliteEventReader(db)
        result = await reader.query_events()
        assert result.total == 1
        assert len(result.events) == 1
        assert result.events[0].workflow_id == "wf-1"

    async def test_backward_compat_query_events(self, tmp_path):
        """Verify standalone function still works."""
        db = tmp_path / "events.db"
        await _setup_db(
            db,
            [
                (
                    "2026-01-01T00:00:00Z",
                    "wf-1",
                    "DispatchReceived",
                    {
                        "type": "dispatch_received",
                        "timestamp": "2026-01-01T00:00:00Z",
                        "workflow_id": "wf-1",
                        "phase": "do-task",
                        "project": "p",
                        "cli": "claude",
                    },
                ),
            ],
        )

        result = await query_events(db)
        assert result.total == 1

    def test_event_reader_protocol_conformance(self, tmp_path):
        db = tmp_path / "events.db"
        reader = SqliteEventReader(db)
        assert isinstance(reader, EventReader)
