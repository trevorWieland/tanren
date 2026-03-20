"""Token usage collection via ccusage tool suite.

Runs ccusage (Claude), @ccusage/codex, or @ccusage/opencode after each dispatch
to collect token usage data. Supports both local and remote (SSH) execution.
"""

from __future__ import annotations

import asyncio
import json
import logging
import shlex
from datetime import UTC, datetime, timedelta
from typing import TYPE_CHECKING, Protocol

from pydantic import BaseModel, ConfigDict, Field

from tanren_core.schemas import Cli

if TYPE_CHECKING:
    from tanren_core.adapters.protocols import RemoteConnection
    from tanren_core.config import Config

logger = logging.getLogger(__name__)

_COLLECTION_TIMEOUT = 30.0


class TokenUsage(BaseModel):
    """Normalized token usage data from any ccusage provider."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    input_tokens: int = Field(..., ge=0)
    output_tokens: int = Field(..., ge=0)
    cache_creation_tokens: int = Field(default=0, ge=0)
    cache_read_tokens: int = Field(default=0, ge=0)
    cached_input_tokens: int = Field(default=0, ge=0)
    reasoning_tokens: int = Field(default=0, ge=0)
    total_tokens: int = Field(..., ge=0)
    total_cost: float = Field(..., ge=0.0)
    models_used: list[str] = Field(default_factory=list)
    provider: str = Field(...)
    session_id: str | None = Field(default=None)


class CommandRunner(Protocol):
    """Abstracts local subprocess vs SSH command execution."""

    async def run_command(self, cmd: list[str], timeout: float) -> tuple[int, str]:  # noqa: ASYNC109
        """Run a command and return (exit_code, stdout).

        Returns:
            Tuple of (exit_code, stdout_text).
        """
        ...


class LocalCommandRunner:
    """Run commands as local subprocesses."""

    async def run_command(self, cmd: list[str], timeout: float) -> tuple[int, str]:  # noqa: ASYNC109
        """Run a command locally via asyncio subprocess.

        Returns:
            Tuple of (exit_code, stdout_text).

        Raises:
            TimeoutError: If the command exceeds the timeout.
        """
        proc = await asyncio.create_subprocess_exec(
            *cmd,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        try:
            stdout_bytes, _ = await asyncio.wait_for(proc.communicate(), timeout=timeout)
        except TimeoutError:
            proc.kill()
            await proc.wait()
            raise
        return proc.returncode or 0, stdout_bytes.decode()


class RemoteCommandRunner:
    """Run commands via an SSH connection."""

    def __init__(self, connection: RemoteConnection, *, run_as_user: str | None = None) -> None:
        """Initialize with an SSH connection object."""
        self._connection = connection
        self._run_as_user = run_as_user

    async def run_command(self, cmd: list[str], timeout: float) -> tuple[int, str]:  # noqa: ASYNC109
        """Run a command on a remote host via SSH.

        Returns:
            Tuple of (exit_code, stdout_text).
        """
        cmd_str = " ".join(shlex.quote(c) for c in cmd)
        if self._run_as_user:
            cmd_str = f"su - {shlex.quote(self._run_as_user)} -c {shlex.quote(cmd_str)}"
        result = await self._connection.run(cmd_str, timeout=timeout)
        return result.exit_code, result.stdout


async def collect_token_usage(
    cli: Cli,
    worktree_path: str,
    dispatch_start_utc: datetime,
    dispatch_end_utc: datetime,
    config: Config,
    runner: CommandRunner,
) -> TokenUsage | None:
    """Collect token usage for a completed dispatch.

    Returns None if the CLI is BASH, or if collection fails for any reason.
    Failures are logged as warnings but never propagate.

    Returns:
        TokenUsage if collection succeeds, None otherwise.
    """
    if cli == Cli.BASH:
        return None

    try:
        if cli == Cli.CLAUDE:
            return await _collect_claude(worktree_path, dispatch_start_utc, config, runner)
        if cli == Cli.CODEX:
            return await _collect_codex(dispatch_start_utc, dispatch_end_utc, config, runner)
        if cli == Cli.OPENCODE:
            return await _collect_opencode(dispatch_start_utc, dispatch_end_utc, config, runner)
        logger.warning("Unknown CLI for token usage collection: %s", cli)
        return None
    except TimeoutError:
        logger.warning("Token usage collection timed out for cli=%s", cli)
        return None
    except Exception:
        logger.warning("Token usage collection failed for cli=%s", cli, exc_info=True)
        return None


async def _collect_claude(
    worktree_path: str,
    dispatch_start_utc: datetime,
    config: Config,
    runner: CommandRunner,
) -> TokenUsage | None:
    """Collect Claude token usage via ccusage.

    Returns:
        TokenUsage if a matching session is found, None otherwise.
    """
    session_id = _derive_session_id(worktree_path)
    since = dispatch_start_utc.strftime("%Y%m%d")

    base_cmd = shlex.split(config.ccusage_claude_cmd)
    cmd = [
        *base_cmd,
        "session",
        "--json",
        "--since",
        since,
        "--id",
        session_id,
        "--offline",
        "--no-color",
    ]

    exit_code, stdout = await runner.run_command(cmd, timeout=_COLLECTION_TIMEOUT)
    if exit_code != 0:
        logger.warning("ccusage (claude) exited with code %d", exit_code)
        return None

    data = json.loads(stdout)
    sessions = data.get("sessions", [])
    if not sessions:
        logger.warning("ccusage (claude) returned no sessions for id=%s", session_id)
        return None

    # Claude --id returns exact match, use first session
    return _normalize_claude(sessions[0])


async def _collect_codex(
    dispatch_start_utc: datetime,
    dispatch_end_utc: datetime,
    config: Config,
    runner: CommandRunner,
) -> TokenUsage | None:
    """Collect Codex token usage via @ccusage/codex.

    Returns:
        TokenUsage if a matching session is found, None otherwise.
    """
    since = dispatch_start_utc.strftime("%Y%m%d")

    base_cmd = shlex.split(config.ccusage_codex_cmd)
    cmd = [*base_cmd, "session", "--json", "--since", since, "--offline", "--noColor"]

    exit_code, stdout = await runner.run_command(cmd, timeout=_COLLECTION_TIMEOUT)
    if exit_code != 0:
        logger.warning("@ccusage/codex exited with code %d", exit_code)
        return None

    data = json.loads(stdout)
    sessions = data.get("sessions", [])
    matched = _match_session_by_time(sessions, dispatch_start_utc, dispatch_end_utc)
    if matched is None:
        logger.warning("@ccusage/codex: no session matched dispatch time window")
        return None

    return _normalize_codex(matched)


async def _collect_opencode(
    dispatch_start_utc: datetime,
    dispatch_end_utc: datetime,
    config: Config,
    runner: CommandRunner,
) -> TokenUsage | None:
    """Collect OpenCode token usage via @ccusage/opencode.

    Returns:
        TokenUsage if a matching session is found, None otherwise.
    """
    base_cmd = shlex.split(config.ccusage_opencode_cmd)
    cmd = [*base_cmd, "session", "--json"]

    exit_code, stdout = await runner.run_command(cmd, timeout=_COLLECTION_TIMEOUT)
    if exit_code != 0:
        logger.warning("@ccusage/opencode exited with code %d", exit_code)
        return None

    data = json.loads(stdout)
    sessions = data.get("sessions", [])
    matched = _match_session_by_time(sessions, dispatch_start_utc, dispatch_end_utc)
    if matched is None:
        logger.warning("@ccusage/opencode: no session matched dispatch time window")
        return None

    return _normalize_opencode(matched)


def _derive_session_id(worktree_path: str) -> str:
    """Derive ccusage session ID from a worktree path.

    Replaces '/' with '-'. E.g. '/home/user/proj' -> '-home-user-proj'.

    Returns:
        Session ID string with slashes replaced by dashes.
    """
    return worktree_path.replace("/", "-")


def _normalize_claude(session: dict) -> TokenUsage:
    """Normalize a Claude ccusage session dict to TokenUsage.

    Returns:
        Normalized TokenUsage instance.
    """
    return TokenUsage(
        input_tokens=session["inputTokens"],
        output_tokens=session["outputTokens"],
        cache_creation_tokens=session.get("cacheCreationTokens", 0),
        cache_read_tokens=session.get("cacheReadTokens", 0),
        cached_input_tokens=0,
        reasoning_tokens=0,
        total_tokens=session["totalTokens"],
        total_cost=session["totalCost"],
        models_used=session.get("modelsUsed", []),
        provider="claude",
        session_id=session.get("sessionId"),
    )


def _normalize_codex(session: dict) -> TokenUsage:
    """Normalize a Codex ccusage session dict to TokenUsage.

    Returns:
        Normalized TokenUsage instance.
    """
    models = session.get("models", {})
    models_list = [str(k) for k in models] if isinstance(models, dict) else []

    return TokenUsage(
        input_tokens=session["inputTokens"],
        output_tokens=session["outputTokens"],
        cache_creation_tokens=0,
        cache_read_tokens=0,
        cached_input_tokens=session.get("cachedInputTokens", 0),
        reasoning_tokens=session.get("reasoningOutputTokens", 0),
        total_tokens=session["totalTokens"],
        total_cost=session["costUSD"],
        models_used=models_list,
        provider="codex",
        session_id=session.get("sessionId"),
    )


def _normalize_opencode(session: dict) -> TokenUsage:
    """Normalize an OpenCode ccusage session dict to TokenUsage.

    Returns:
        Normalized TokenUsage instance.
    """
    return TokenUsage(
        input_tokens=session["inputTokens"],
        output_tokens=session["outputTokens"],
        cache_creation_tokens=session.get("cacheCreationTokens", 0),
        cache_read_tokens=session.get("cacheReadTokens", 0),
        cached_input_tokens=0,
        reasoning_tokens=0,
        total_tokens=session["totalTokens"],
        total_cost=session["totalCost"],
        models_used=session.get("modelsUsed", []),
        provider="opencode",
        session_id=session.get("sessionID"),
    )


def _match_session_by_time(
    sessions: list[dict],
    start_utc: datetime,
    end_utc: datetime,
) -> dict | None:
    """Find the session whose lastActivity falls within the dispatch window.

    Window is [start_utc - 60s, end_utc + 60s].

    Returns:
        The session closest to end_utc, or None if no match.
    """
    window_start = start_utc - timedelta(seconds=60)
    window_end = end_utc + timedelta(seconds=60)
    best: dict | None = None
    best_distance: float = float("inf")

    for session in sessions:
        activity_str = session.get("lastActivity", "")
        if not activity_str:
            continue

        activity = _parse_activity_time(activity_str)
        if activity is None:
            continue

        if window_start <= activity <= window_end:
            distance = abs((activity - end_utc).total_seconds())
            if distance < best_distance:
                best = session
                best_distance = distance

    return best


def _parse_activity_time(s: str) -> datetime | None:
    """Parse a lastActivity string (date-only or ISO timestamp).

    Returns:
        Parsed datetime or None if parsing fails.
    """
    try:
        # ISO timestamp with timezone (e.g. "2026-03-13T12:25:32.843Z")
        if "T" in s:
            # Handle 'Z' suffix
            cleaned = s.replace("Z", "+00:00")
            return datetime.fromisoformat(cleaned)
        # Date only (e.g. "2026-03-14") — treat as midnight UTC
        return datetime.strptime(s, "%Y-%m-%d").replace(tzinfo=UTC)
    except (ValueError, TypeError):
        return None
