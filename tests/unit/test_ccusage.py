"""Unit tests for tanren_core.ccusage — token usage collection."""

from __future__ import annotations

import json
from datetime import UTC, datetime
from unittest.mock import AsyncMock, patch

import pytest

from tanren_core.ccusage import (
    LocalCommandRunner,
    _derive_session_id,  # noqa: PLC2701
    _match_session_by_time,  # noqa: PLC2701
    _normalize_claude,  # noqa: PLC2701
    _normalize_codex,  # noqa: PLC2701
    _normalize_opencode,  # noqa: PLC2701
    collect_token_usage,
)
from tanren_core.config import Config
from tanren_core.schemas import Cli

# ---------------------------------------------------------------------------
# Fixtures — real-shaped ccusage JSON
# ---------------------------------------------------------------------------

CLAUDE_SESSION = {
    "sessionId": "-home-trevor-github-tanren",
    "inputTokens": 33653,
    "outputTokens": 193856,
    "cacheCreationTokens": 5336560,
    "cacheReadTokens": 177649313,
    "totalTokens": 183213382,
    "totalCost": 127.19,
    "lastActivity": "2026-03-14",
    "modelsUsed": ["claude-opus-4-6"],
    "modelBreakdowns": [],
    "projectPath": "/home/trevor/github/tanren",
}

CODEX_SESSION = {
    "sessionId": "2026/03/13/rollout-abc-123",
    "lastActivity": "2026-03-13T12:25:32.843Z",
    "inputTokens": 2841753,
    "cachedInputTokens": 2753536,
    "outputTokens": 13534,
    "reasoningOutputTokens": 9297,
    "totalTokens": 2855287,
    "costUSD": 0.826,
    "models": {"gpt-5.3-codex": {"inputTokens": 2841753}},
}

OPENCODE_SESSION = {
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
    "lastActivity": "2026-01-23T16:37:20.972Z",
}


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _make_config() -> Config:
    """Build a minimal Config with ccusage command defaults."""
    return Config(
        ipc_dir="/tmp/ipc",
        github_dir="/tmp/gh",
        data_dir="/tmp/data",
        commands_dir=".claude/commands/tanren",
        worktree_registry_path="/tmp/worktrees.json",
        roles_config_path="/tmp/roles.yml",
    )


def _make_runner(exit_code: int = 0, stdout: str = "{}") -> AsyncMock:
    """Build a mock CommandRunner."""
    runner = AsyncMock()
    runner.run_command = AsyncMock(return_value=(exit_code, stdout))
    return runner


# ---------------------------------------------------------------------------
# _derive_session_id
# ---------------------------------------------------------------------------


class TestDeriveSessionId:
    def test_basic_path(self):
        assert _derive_session_id("/home/trevor/github/proj") == "-home-trevor-github-proj"

    def test_root(self):
        assert _derive_session_id("/") == "-"

    def test_workspace(self):
        assert _derive_session_id("/workspace/proj") == "-workspace-proj"


# ---------------------------------------------------------------------------
# _normalize_claude
# ---------------------------------------------------------------------------


class TestNormalizeClaude:
    def test_normalizes_fields(self):
        usage = _normalize_claude(CLAUDE_SESSION)
        assert usage.input_tokens == 33653
        assert usage.output_tokens == 193856
        assert usage.cache_creation_tokens == 5336560
        assert usage.cache_read_tokens == 177649313
        assert usage.cached_input_tokens == 0
        assert usage.reasoning_tokens == 0
        assert usage.total_tokens == 183213382
        assert usage.total_cost == pytest.approx(127.19)
        assert usage.models_used == ["claude-opus-4-6"]
        assert usage.provider == "claude"
        assert usage.session_id == "-home-trevor-github-tanren"


# ---------------------------------------------------------------------------
# _normalize_codex
# ---------------------------------------------------------------------------


class TestNormalizeCodex:
    def test_normalizes_fields(self):
        usage = _normalize_codex(CODEX_SESSION)
        assert usage.input_tokens == 2841753
        assert usage.output_tokens == 13534
        assert usage.cache_creation_tokens == 0
        assert usage.cache_read_tokens == 0
        assert usage.cached_input_tokens == 2753536
        assert usage.reasoning_tokens == 9297
        assert usage.total_tokens == 2855287
        assert usage.total_cost == pytest.approx(0.826)
        assert usage.models_used == ["gpt-5.3-codex"]
        assert usage.provider == "codex"
        assert usage.session_id == "2026/03/13/rollout-abc-123"


# ---------------------------------------------------------------------------
# _normalize_opencode
# ---------------------------------------------------------------------------


class TestNormalizeOpencode:
    def test_normalizes_fields(self):
        usage = _normalize_opencode(OPENCODE_SESSION)
        assert usage.input_tokens == 16116
        assert usage.output_tokens == 1566
        assert usage.cache_creation_tokens == 0
        assert usage.cache_read_tokens == 64384
        assert usage.cached_input_tokens == 0
        assert usage.reasoning_tokens == 0
        assert usage.total_tokens == 82066
        assert usage.total_cost == pytest.approx(0.0614)
        assert usage.models_used == ["gpt-5.2-codex"]
        assert usage.provider == "opencode"
        assert usage.session_id == "ses_41448dad"


# ---------------------------------------------------------------------------
# _match_session_by_time
# ---------------------------------------------------------------------------


class TestMatchSessionByTime:
    def test_within_window(self):
        start = datetime(2026, 3, 13, 12, 20, 0, tzinfo=UTC)
        end = datetime(2026, 3, 13, 12, 30, 0, tzinfo=UTC)
        sessions = [CODEX_SESSION]
        matched = _match_session_by_time(sessions, start, end)
        assert matched is not None
        assert matched["sessionId"] == "2026/03/13/rollout-abc-123"

    def test_no_match(self):
        start = datetime(2026, 1, 1, 0, 0, 0, tzinfo=UTC)
        end = datetime(2026, 1, 1, 0, 5, 0, tzinfo=UTC)
        sessions = [CODEX_SESSION]
        matched = _match_session_by_time(sessions, start, end)
        assert matched is None

    def test_multiple_sessions_picks_closest_to_end(self):
        start = datetime(2026, 1, 23, 16, 30, 0, tzinfo=UTC)
        end = datetime(2026, 1, 23, 16, 40, 0, tzinfo=UTC)
        earlier = {
            "lastActivity": "2026-01-23T16:32:00.000Z",
            "sessionId": "earlier",
        }
        closer = {
            "lastActivity": "2026-01-23T16:38:00.000Z",
            "sessionId": "closer",
        }
        matched = _match_session_by_time([earlier, closer], start, end)
        assert matched is not None
        assert matched["sessionId"] == "closer"

    def test_empty_sessions(self):
        start = datetime(2026, 1, 1, 0, 0, 0, tzinfo=UTC)
        end = datetime(2026, 1, 1, 0, 5, 0, tzinfo=UTC)
        assert _match_session_by_time([], start, end) is None


# ---------------------------------------------------------------------------
# collect_token_usage — edge cases
# ---------------------------------------------------------------------------


class TestCollectTokenUsage:
    @pytest.mark.asyncio
    async def test_returns_none_for_bash(self):
        runner = _make_runner()
        config = _make_config()
        result = await collect_token_usage(
            Cli.BASH,
            "/tmp/wt",
            datetime.now(UTC),
            datetime.now(UTC),
            config,
            runner,
        )
        assert result is None
        runner.run_command.assert_not_called()

    @pytest.mark.asyncio
    async def test_returns_none_on_subprocess_failure(self):
        runner = _make_runner(exit_code=1, stdout="error")
        config = _make_config()
        result = await collect_token_usage(
            Cli.CLAUDE,
            "/tmp/wt",
            datetime.now(UTC),
            datetime.now(UTC),
            config,
            runner,
        )
        assert result is None

    @pytest.mark.asyncio
    async def test_returns_none_on_timeout(self):
        runner = AsyncMock()
        runner.run_command = AsyncMock(side_effect=TimeoutError("timed out"))
        config = _make_config()
        result = await collect_token_usage(
            Cli.CLAUDE,
            "/tmp/wt",
            datetime.now(UTC),
            datetime.now(UTC),
            config,
            runner,
        )
        assert result is None

    @pytest.mark.asyncio
    async def test_returns_none_on_json_parse_error(self):
        runner = _make_runner(exit_code=0, stdout="not json at all")
        config = _make_config()
        result = await collect_token_usage(
            Cli.CLAUDE,
            "/tmp/wt",
            datetime.now(UTC),
            datetime.now(UTC),
            config,
            runner,
        )
        assert result is None

    @pytest.mark.asyncio
    async def test_happy_path_claude(self):
        payload = json.dumps({"sessions": [CLAUDE_SESSION]})
        runner = _make_runner(exit_code=0, stdout=payload)
        config = _make_config()
        result = await collect_token_usage(
            Cli.CLAUDE,
            "/home/trevor/github/tanren",
            datetime(2026, 3, 14, 0, 0, 0, tzinfo=UTC),
            datetime(2026, 3, 14, 1, 0, 0, tzinfo=UTC),
            config,
            runner,
        )
        assert result is not None
        assert result.provider == "claude"
        assert result.total_cost == pytest.approx(127.19)

    @pytest.mark.asyncio
    async def test_happy_path_codex(self):
        payload = json.dumps({"sessions": [CODEX_SESSION]})
        runner = _make_runner(exit_code=0, stdout=payload)
        config = _make_config()
        result = await collect_token_usage(
            Cli.CODEX,
            "/tmp/wt",
            datetime(2026, 3, 13, 12, 20, 0, tzinfo=UTC),
            datetime(2026, 3, 13, 12, 30, 0, tzinfo=UTC),
            config,
            runner,
        )
        assert result is not None
        assert result.provider == "codex"
        assert result.cached_input_tokens == 2753536
        assert result.reasoning_tokens == 9297

    @pytest.mark.asyncio
    async def test_happy_path_opencode(self):
        payload = json.dumps({"sessions": [OPENCODE_SESSION]})
        runner = _make_runner(exit_code=0, stdout=payload)
        config = _make_config()
        result = await collect_token_usage(
            Cli.OPENCODE,
            "/tmp/wt",
            datetime(2026, 1, 23, 16, 30, 0, tzinfo=UTC),
            datetime(2026, 1, 23, 16, 40, 0, tzinfo=UTC),
            config,
            runner,
        )
        assert result is not None
        assert result.provider == "opencode"
        assert result.session_id == "ses_41448dad"


# ---------------------------------------------------------------------------
# LocalCommandRunner — orphaned process kill on timeout
# ---------------------------------------------------------------------------


class TestLocalCommandRunnerTimeout:
    @pytest.mark.asyncio
    async def test_kills_process_on_timeout(self):
        """LocalCommandRunner kills the subprocess when timeout fires."""
        mock_proc = AsyncMock()
        mock_proc.communicate = AsyncMock(side_effect=TimeoutError)
        mock_proc.kill = AsyncMock()
        mock_proc.wait = AsyncMock()

        with patch("tanren_core.ccusage.asyncio.create_subprocess_exec", return_value=mock_proc):
            runner = LocalCommandRunner()
            with pytest.raises(TimeoutError):
                await runner.run_command(["sleep", "999"], timeout=0.01)

        mock_proc.kill.assert_called_once()
        mock_proc.wait.assert_awaited_once()
