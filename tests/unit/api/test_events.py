"""Tests for events endpoint."""

from __future__ import annotations

import json
from unittest.mock import AsyncMock

import pytest

from tanren_core.adapters.event_reader import EventQueryResult, EventRow
from tanren_core.adapters.events import DispatchReceived


@pytest.mark.api
class TestEvents:
    async def test_events_no_db_returns_empty(self, client, auth_headers):
        resp = await client.get("/api/v1/events", headers=auth_headers)
        assert resp.status_code == 200
        data = resp.json()
        assert data["events"] == []
        assert data["total"] == 0

    async def test_events_with_db(self, client, auth_headers, app, sqlite_store):
        await sqlite_store.append(
            DispatchReceived(
                timestamp="2026-01-01T00:00:00Z",
                workflow_id="wf-1",
                phase="do-task",
                project="p",
                cli="claude",
            )
        )

        resp = await client.get("/api/v1/events", headers=auth_headers)
        assert resp.status_code == 200
        data = resp.json()
        assert data["total"] == 1
        assert len(data["events"]) == 1
        assert data["events"][0]["type"] == "dispatch_received"

    async def test_events_pagination(self, client, auth_headers, app, sqlite_store):
        for i in range(5):
            await sqlite_store.append(
                DispatchReceived(
                    timestamp=f"2026-01-01T00:00:{i:02d}Z",
                    workflow_id="wf-1",
                    phase="do-task",
                    project="p",
                    cli="claude",
                )
            )

        resp = await client.get("/api/v1/events?limit=2&offset=0", headers=auth_headers)
        data = resp.json()
        assert data["total"] == 5
        assert len(data["events"]) == 2

    async def test_events_skipped_count(self, client, auth_headers, app, sqlite_store):
        await sqlite_store.append(
            DispatchReceived(
                timestamp="2026-01-01T00:00:00Z",
                workflow_id="wf-1",
                phase="do-task",
                project="p",
                cli="claude",
            )
        )
        # Insert a row with an invalid event type directly to simulate a bad event
        conn = await sqlite_store._ensure_conn()
        await conn.execute("BEGIN IMMEDIATE")
        await conn.execute(
            "INSERT INTO events (event_id, timestamp, workflow_id, event_type, payload) "
            "VALUES (?, ?, ?, ?, ?)",
            (
                "bad-event-id",
                "2026-01-01T00:00:01Z",
                "wf-1",
                "BadEvent",
                json.dumps({"type": "nonexistent_type", "garbage": True}),
            ),
        )
        await conn.commit()

        resp = await client.get("/api/v1/events", headers=auth_headers)
        assert resp.status_code == 200
        data = resp.json()
        assert data["skipped"] == 1
        assert len(data["events"]) == 1
        assert data["total"] == 2  # raw DB count, not adjusted by skipped

    async def test_events_filter_workflow_id(self, client, auth_headers, app, sqlite_store):
        await sqlite_store.append(
            DispatchReceived(
                timestamp="2026-01-01T00:00:00Z",
                workflow_id="wf-1",
                phase="do-task",
                project="p",
                cli="claude",
            )
        )
        await sqlite_store.append(
            DispatchReceived(
                timestamp="2026-01-01T00:00:01Z",
                workflow_id="wf-2",
                phase="do-task",
                project="q",
                cli="claude",
            )
        )

        resp = await client.get("/api/v1/events?workflow_id=wf-1", headers=auth_headers)
        data = resp.json()
        assert data["total"] == 1
        assert data["events"][0]["workflow_id"] == "wf-1"

    async def test_events_malformed_json_payload_returns_200(
        self, client, auth_headers, app, sqlite_store
    ):
        await sqlite_store.append(
            DispatchReceived(
                timestamp="2026-01-01T00:00:00Z",
                workflow_id="wf-1",
                phase="do-task",
                project="p",
                cli="claude",
            )
        )
        # Insert malformed JSON and invalid schema rows directly
        conn = await sqlite_store._ensure_conn()
        await conn.execute("BEGIN IMMEDIATE")
        await conn.execute(
            "INSERT INTO events (event_id, timestamp, workflow_id, event_type, payload) "
            "VALUES (?, ?, ?, ?, ?)",
            ("bad-json-id", "2026-01-01T00:00:01Z", "wf-1", "Bad", "not json {{"),
        )
        await conn.execute(
            "INSERT INTO events (event_id, timestamp, workflow_id, event_type, payload) "
            "VALUES (?, ?, ?, ?, ?)",
            (
                "bad-schema-id",
                "2026-01-01T00:00:02Z",
                "wf-1",
                "BadEvent",
                json.dumps({"type": "nonexistent_type", "garbage": True}),
            ),
        )
        await conn.commit()

        resp = await client.get("/api/v1/events", headers=auth_headers)
        assert resp.status_code == 200
        data = resp.json()
        assert data["total"] == 3
        # 1 malformed JSON (skipped in store) + 1 invalid schema (skipped in service)
        assert data["skipped"] == 2
        assert len(data["events"]) == 1
        assert data["events"][0]["type"] == "dispatch_received"


@pytest.mark.api
class TestEventsWithInjectedReader:
    async def test_events_uses_injected_event_store(self, client, auth_headers, app):
        """Regression: /events endpoint must use event_store from app state."""
        mock_store = AsyncMock()
        mock_store.query_events = AsyncMock(
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
        app.state.event_store = mock_store

        resp = await client.get("/api/v1/events", headers=auth_headers)
        assert resp.status_code == 200
        data = resp.json()
        assert data["total"] == 1
        assert data["events"][0]["workflow_id"] == "wf-pg"
        mock_store.query_events.assert_awaited_once()
