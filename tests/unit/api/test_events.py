"""Tests for events endpoint."""

from __future__ import annotations

import json
from unittest.mock import AsyncMock

import aiosqlite
import pytest

from tanren_core.adapters.event_reader import EventQueryResult, EventRow
from tanren_core.adapters.sqlite_emitter import (
    _SCHEMA,
)


async def _setup_events_db(db_path, events: list[tuple[str, str, str, dict]]):
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


async def _setup_events_db_raw(db_path, events: list[tuple[str, str, str, str]]):
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


@pytest.mark.api
class TestEvents:
    async def test_events_no_db_returns_empty(self, client, auth_headers):
        resp = await client.get("/api/v1/events", headers=auth_headers)
        assert resp.status_code == 200
        data = resp.json()
        assert data["events"] == []
        assert data["total"] == 0

    async def test_events_with_db(self, client, auth_headers, app, tmp_path):
        db = tmp_path / "events.db"
        await _setup_events_db(
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
        app.state.settings.events_db = str(db)

        resp = await client.get("/api/v1/events", headers=auth_headers)
        assert resp.status_code == 200
        data = resp.json()
        assert data["total"] == 1
        assert len(data["events"]) == 1
        assert data["events"][0]["type"] == "dispatch_received"

    async def test_events_pagination(self, client, auth_headers, app, tmp_path):
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
            for i in range(5)
        ]
        await _setup_events_db(db, events)
        app.state.settings.events_db = str(db)

        resp = await client.get("/api/v1/events?limit=2&offset=0", headers=auth_headers)
        data = resp.json()
        assert data["total"] == 5
        assert len(data["events"]) == 2

    async def test_events_skipped_count(self, client, auth_headers, app, tmp_path):
        db = tmp_path / "events.db"
        await _setup_events_db(
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
                    "BadEvent",
                    {"type": "nonexistent_type", "garbage": True},
                ),
            ],
        )
        app.state.settings.events_db = str(db)

        resp = await client.get("/api/v1/events", headers=auth_headers)
        assert resp.status_code == 200
        data = resp.json()
        assert data["skipped"] == 1
        assert len(data["events"]) == 1
        assert data["total"] == 2  # raw DB count, not adjusted by skipped

    async def test_events_filter_workflow_id(self, client, auth_headers, app, tmp_path):
        db = tmp_path / "events.db"
        await _setup_events_db(
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
        app.state.settings.events_db = str(db)

        resp = await client.get("/api/v1/events?workflow_id=wf-1", headers=auth_headers)
        data = resp.json()
        assert data["total"] == 1
        assert data["events"][0]["workflow_id"] == "wf-1"

    async def test_events_malformed_json_payload_returns_200(
        self, client, auth_headers, app, tmp_path
    ):
        db = tmp_path / "events.db"
        valid_payload = json.dumps({
            "type": "dispatch_received",
            "timestamp": "2026-01-01T00:00:00Z",
            "workflow_id": "wf-1",
            "phase": "do-task",
            "project": "p",
            "cli": "claude",
        })
        await _setup_events_db_raw(
            db,
            [
                ("2026-01-01T00:00:00Z", "wf-1", "DispatchReceived", valid_payload),
                ("2026-01-01T00:00:01Z", "wf-1", "Bad", "not json {{"),
                (
                    "2026-01-01T00:00:02Z",
                    "wf-1",
                    "BadEvent",
                    json.dumps({
                        "type": "nonexistent_type",
                        "garbage": True,
                    }),
                ),
            ],
        )
        app.state.settings.events_db = str(db)

        resp = await client.get("/api/v1/events", headers=auth_headers)
        assert resp.status_code == 200
        data = resp.json()
        assert data["total"] == 3
        # 1 malformed JSON (skipped in event_reader) + 1 invalid schema (skipped in router)
        assert data["skipped"] == 2
        assert len(data["events"]) == 1
        assert data["events"][0]["type"] == "dispatch_received"


@pytest.mark.api
class TestEventsWithInjectedReader:
    async def test_events_uses_injected_event_reader(self, client, auth_headers, app):
        """Regression: /events endpoint must use event_reader from app state.

        Without this, Postgres DSNs would be treated as SQLite paths and silently
        return empty results.
        """
        mock_reader = AsyncMock()
        mock_reader.query_events = AsyncMock(
            return_value=EventQueryResult(
                events=[
                    EventRow(
                        id=1,
                        timestamp="2026-01-01T00:00:00Z",
                        workflow_id="wf-pg",
                        event_type="DispatchReceived",
                        payload={
                            "type": "dispatch_received",
                            "timestamp": "2026-01-01T00:00:00Z",
                            "workflow_id": "wf-pg",
                            "phase": "do-task",
                            "project": "p",
                            "cli": "claude",
                        },
                    )
                ],
                total=1,
                skipped=0,
            )
        )
        app.state.event_reader = mock_reader

        resp = await client.get("/api/v1/events", headers=auth_headers)
        assert resp.status_code == 200
        data = resp.json()
        assert data["total"] == 1
        assert data["events"][0]["workflow_id"] == "wf-pg"
        mock_reader.query_events.assert_awaited_once()
