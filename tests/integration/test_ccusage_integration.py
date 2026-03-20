"""Integration tests for ccusage token usage collection."""

from __future__ import annotations

import json
from datetime import UTC, datetime
from typing import TYPE_CHECKING
from unittest.mock import AsyncMock

import aiosqlite
import pytest
from pydantic import TypeAdapter

from tanren_api.models import EventPayload
from tanren_core.adapters.events import TokenUsageRecorded
from tanren_core.adapters.sqlite_emitter import SqliteEventEmitter
from tanren_core.ccusage import (
    TokenUsage,
    collect_token_usage,
)
from tanren_core.config import Config
from tanren_core.schemas import Cli, Outcome, Phase, Result

if TYPE_CHECKING:
    from pathlib import Path

# ---------------------------------------------------------------------------
# Fixtures — real-shaped JSON from ccusage tools
# ---------------------------------------------------------------------------

CLAUDE_FIXTURE = {
    "sessions": [
        {
            "sessionId": "-workspace-proj",
            "inputTokens": 33653,
            "outputTokens": 193856,
            "cacheCreationTokens": 5336560,
            "cacheReadTokens": 177649313,
            "totalTokens": 183213382,
            "totalCost": 127.19,
            "lastActivity": "2026-03-14",
            "modelsUsed": ["claude-opus-4-6"],
            "modelBreakdowns": [],
            "projectPath": "/workspace/proj",
        }
    ]
}

CODEX_FIXTURE = {
    "sessions": [
        {
            "sessionId": "2026/03/14/rollout-abc-123",
            "lastActivity": "2026-03-14T10:25:32.843Z",
            "inputTokens": 2841753,
            "cachedInputTokens": 2753536,
            "outputTokens": 13534,
            "reasoningOutputTokens": 9297,
            "totalTokens": 2855287,
            "costUSD": 0.826,
            "models": {"gpt-5.3-codex": {"inputTokens": 2841753}},
        }
    ]
}

OPENCODE_FIXTURE = {
    "sessions": [
        {
            "sessionID": "ses_41448dad",
            "sessionTitle": "Fix tests",
            "parentID": None,
            "inputTokens": 16116,
            "outputTokens": 1566,
            "cacheCreationTokens": 0,
            "cacheReadTokens": 64384,
            "totalTokens": 82066,
            "totalCost": 0.0614,
            "modelsUsed": ["gpt-5.2-codex"],
            "lastActivity": "2026-03-14T10:37:20.972Z",
        }
    ]
}


def _make_config() -> Config:
    return Config(
        ipc_dir="/tmp/ipc",
        github_dir="/tmp/gh",
        data_dir="/tmp/data",
        commands_dir=".claude/commands/tanren",
        worktree_registry_path="/tmp/worktrees.json",
        roles_config_path="/tmp/roles.yml",
    )


def _make_mock_runner(fixture: dict) -> AsyncMock:
    """Build a mock CommandRunner that returns fixture JSON."""
    runner = AsyncMock()
    runner.run_command = AsyncMock(return_value=(0, json.dumps(fixture)))
    return runner


# ---------------------------------------------------------------------------
# Full collection path per provider (mocked SSH)
# ---------------------------------------------------------------------------


class TestCollectTokenUsageIntegration:
    @pytest.mark.asyncio
    async def test_claude_via_mock_remote(self):
        runner = _make_mock_runner(CLAUDE_FIXTURE)
        config = _make_config()
        result = await collect_token_usage(
            Cli.CLAUDE,
            "/workspace/proj",
            datetime(2026, 3, 14, 9, 0, 0, tzinfo=UTC),
            datetime(2026, 3, 14, 10, 0, 0, tzinfo=UTC),
            config,
            runner,
        )
        assert result is not None
        assert result.provider == "claude"
        assert result.total_cost == pytest.approx(127.19)
        assert result.session_id == "-workspace-proj"
        assert result.models_used == ["claude-opus-4-6"]

    @pytest.mark.asyncio
    async def test_codex_via_mock_remote(self):
        runner = _make_mock_runner(CODEX_FIXTURE)
        config = _make_config()
        result = await collect_token_usage(
            Cli.CODEX,
            "/workspace/proj",
            datetime(2026, 3, 14, 10, 20, 0, tzinfo=UTC),
            datetime(2026, 3, 14, 10, 30, 0, tzinfo=UTC),
            config,
            runner,
        )
        assert result is not None
        assert result.provider == "codex"
        assert result.cached_input_tokens == 2753536
        assert result.reasoning_tokens == 9297
        assert result.total_cost == pytest.approx(0.826)

    @pytest.mark.asyncio
    async def test_opencode_via_mock_remote(self):
        runner = _make_mock_runner(OPENCODE_FIXTURE)
        config = _make_config()
        result = await collect_token_usage(
            Cli.OPENCODE,
            "/workspace/proj",
            datetime(2026, 3, 14, 10, 30, 0, tzinfo=UTC),
            datetime(2026, 3, 14, 10, 40, 0, tzinfo=UTC),
            config,
            runner,
        )
        assert result is not None
        assert result.provider == "opencode"
        assert result.session_id == "ses_41448dad"
        assert result.total_cost == pytest.approx(0.0614)


# ---------------------------------------------------------------------------
# Event round-trip through SqliteEventEmitter
# ---------------------------------------------------------------------------


class TestTokenUsageEventRoundTrip:
    @pytest.mark.asyncio
    async def test_emit_and_read_back(self, tmp_path: Path):
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
            session_id="-workspace-proj",
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
            assert payload["type"] == "token_usage_recorded"


# ---------------------------------------------------------------------------
# Result with token_usage serializes/deserializes
# ---------------------------------------------------------------------------


class TestResultWithTokenUsage:
    def test_result_round_trip(self):
        usage = TokenUsage(
            input_tokens=1000,
            output_tokens=500,
            total_tokens=1500,
            total_cost=0.05,
            models_used=["claude-opus-4-6"],
            provider="claude",
            session_id="test-session",
        )
        result = Result(
            workflow_id="wf-proj-1-1234",
            phase=Phase.DO_TASK,
            outcome=Outcome.SUCCESS,
            exit_code=0,
            duration_secs=120,
            spec_modified=False,
            token_usage=usage.model_dump(mode="json"),
        )
        data = result.model_dump(mode="json")
        assert data["token_usage"] is not None
        assert data["token_usage"]["provider"] == "claude"

        restored = Result.model_validate(data)
        assert restored.token_usage is not None
        assert restored.token_usage["provider"] == "claude"

    def test_result_without_token_usage(self):
        result = Result(
            workflow_id="wf-proj-1-1234",
            phase=Phase.DO_TASK,
            outcome=Outcome.SUCCESS,
            exit_code=0,
            duration_secs=120,
            spec_modified=False,
        )
        data = result.model_dump(mode="json")
        assert data["token_usage"] is None


# ---------------------------------------------------------------------------
# EventPayload union accepts TokenUsageRecorded
# ---------------------------------------------------------------------------


class TestEventPayloadUnion:
    def test_accepts_token_usage_recorded(self):
        adapter = TypeAdapter(EventPayload)
        event = TokenUsageRecorded(
            timestamp="2026-03-14T10:00:00Z",
            workflow_id="wf-proj-1-1234",
            phase="do-task",
            project="proj",
            cli="claude",
            input_tokens=1000,
            output_tokens=500,
            total_tokens=1500,
            total_cost=0.05,
        )
        data = event.model_dump()
        restored = adapter.validate_python(data)
        assert type(restored) is TokenUsageRecorded
        assert restored.total_cost == pytest.approx(0.05)
