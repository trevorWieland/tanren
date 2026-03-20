"""Tests for SqliteEventEmitter."""

import json
from typing import TYPE_CHECKING

import aiosqlite
import pytest

from tanren_core.adapters.events import DispatchReceived, Event, PhaseCompleted, TokenUsageRecorded
from tanren_core.adapters.sqlite_emitter import SqliteEventEmitter

if TYPE_CHECKING:
    from pathlib import Path


class TestSqliteEventEmitter:
    @pytest.mark.asyncio
    async def test_creates_db_and_table(self, tmp_path: Path):
        db_path = tmp_path / "events.db"
        emitter = SqliteEventEmitter(db_path)
        event = Event(timestamp="2026-01-01T00:00:00Z", workflow_id="wf-test-1-1000")
        await emitter.emit(event)
        await emitter.close()

        assert db_path.exists()
        async with aiosqlite.connect(str(db_path)) as conn:
            cursor = await conn.execute("SELECT COUNT(*) FROM events")
            row = await cursor.fetchone()
            assert row is not None
            assert row[0] == 1

    @pytest.mark.asyncio
    async def test_inserts_correct_data(self, tmp_path: Path):
        db_path = tmp_path / "events.db"
        emitter = SqliteEventEmitter(db_path)
        event = DispatchReceived(
            timestamp="2026-03-10T12:00:00Z",
            workflow_id="wf-rentl-42-1710000000",
            phase="do-task",
            project="rentl",
            cli="opencode",
        )
        await emitter.emit(event)
        await emitter.close()

        async with aiosqlite.connect(str(db_path)) as conn:
            cursor = await conn.execute(
                "SELECT timestamp, workflow_id, event_type, payload FROM events"
            )
            row = await cursor.fetchone()
            assert row is not None
            assert row[0] == "2026-03-10T12:00:00Z"
            assert row[1] == "wf-rentl-42-1710000000"
            assert row[2] == "DispatchReceived"
            payload = json.loads(row[3])
            assert payload["phase"] == "do-task"
            assert payload["project"] == "rentl"
            assert payload["cli"] == "opencode"

    @pytest.mark.asyncio
    async def test_multiple_events(self, tmp_path: Path):
        db_path = tmp_path / "events.db"
        emitter = SqliteEventEmitter(db_path)
        for i in range(5):
            await emitter.emit(
                Event(timestamp=f"2026-01-01T00:00:0{i}Z", workflow_id=f"wf-test-{i}-1000")
            )
        await emitter.close()

        async with aiosqlite.connect(str(db_path)) as conn:
            cursor = await conn.execute("SELECT COUNT(*) FROM events")
            row = await cursor.fetchone()
            assert row is not None
            assert row[0] == 5

    @pytest.mark.asyncio
    async def test_lazy_connection(self, tmp_path: Path):
        db_path = tmp_path / "events.db"
        emitter = SqliteEventEmitter(db_path)
        # DB should not exist yet
        assert not db_path.exists()
        await emitter.close()
        # Still should not exist — never emitted
        assert not db_path.exists()

    @pytest.mark.asyncio
    async def test_creates_parent_dirs(self, tmp_path: Path):
        db_path = tmp_path / "subdir" / "deep" / "events.db"
        emitter = SqliteEventEmitter(db_path)
        await emitter.emit(Event(timestamp="2026-01-01T00:00:00Z", workflow_id="wf-test-1-1000"))
        await emitter.close()
        assert db_path.exists()

    @pytest.mark.asyncio
    async def test_close_idempotent(self, tmp_path: Path):
        db_path = tmp_path / "events.db"
        emitter = SqliteEventEmitter(db_path)
        await emitter.emit(Event(timestamp="2026-01-01T00:00:00Z", workflow_id="wf-test-1-1000"))
        await emitter.close()
        await emitter.close()  # Should not raise

    @pytest.mark.asyncio
    async def test_phase_completed_event(self, tmp_path: Path):
        db_path = tmp_path / "events.db"
        emitter = SqliteEventEmitter(db_path)
        event = PhaseCompleted(
            timestamp="2026-03-10T12:05:00Z",
            workflow_id="wf-rentl-42-1710000000",
            phase="do-task",
            project="rentl",
            outcome="success",
            signal="do-task-status: complete",
            duration_secs=120,
            exit_code=0,
        )
        await emitter.emit(event)
        await emitter.close()

        async with aiosqlite.connect(str(db_path)) as conn:
            cursor = await conn.execute("SELECT event_type, payload FROM events")
            row = await cursor.fetchone()
            assert row is not None
            assert row[0] == "PhaseCompleted"
            payload = json.loads(row[1])
            assert payload["duration_secs"] == 120
            assert payload["outcome"] == "success"

    @pytest.mark.asyncio
    async def test_token_usage_recorded_event(self, tmp_path: Path):
        db_path = tmp_path / "events.db"
        emitter = SqliteEventEmitter(db_path)
        event = TokenUsageRecorded(
            timestamp="2026-03-14T10:00:00Z",
            workflow_id="wf-proj-1-1234",
            phase="do-task",
            project="proj",
            cli="claude",
            input_tokens=33653,
            output_tokens=193856,
            cache_creation_tokens=5336560,
            cache_read_tokens=177649313,
            total_tokens=183213382,
            total_cost=127.19,
            models_used=["claude-opus-4-6"],
            session_id="-home-trevor-github-tanren",
        )
        await emitter.emit(event)
        await emitter.close()

        async with aiosqlite.connect(str(db_path)) as conn:
            cursor = await conn.execute("SELECT event_type, payload FROM events")
            row = await cursor.fetchone()
            assert row is not None
            assert row[0] == "TokenUsageRecorded"
            payload = json.loads(row[1])
            assert payload["total_cost"] == pytest.approx(127.19)
            assert payload["cli"] == "claude"
            assert payload["models_used"] == ["claude-opus-4-6"]
