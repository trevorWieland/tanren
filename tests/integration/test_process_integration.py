"""Integration tests for process.py — prompt assembly, spawn command construction, and timeout."""

from __future__ import annotations

from typing import TYPE_CHECKING
from unittest.mock import AsyncMock, patch

import pytest

from tanren_core.config import Config
from tanren_core.process import (
    ProcessResult,
    _run_with_timeout,  # noqa: PLC2701
    _spawn_bash,  # noqa: PLC2701
    _spawn_claude,  # noqa: PLC2701
    _spawn_codex,  # noqa: PLC2701
    _spawn_opencode,  # noqa: PLC2701
    assemble_prompt,
    spawn_process,
)
from tanren_core.schemas import Cli, Dispatch, Phase

if TYPE_CHECKING:
    from pathlib import Path

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _make_config(tmp_path: Path) -> Config:
    return Config(
        ipc_dir=str(tmp_path),
        github_dir=str(tmp_path),
        data_dir=str(tmp_path),
        worktree_registry_path=str(tmp_path / "worktrees.json"),
        opencode_path="/usr/local/bin/opencode",
        codex_path="/usr/local/bin/codex",
        claude_path="/usr/local/bin/claude",
        roles_config_path=str(tmp_path / "roles.yml"),
    )


def _make_dispatch(
    cli: Cli,
    *,
    phase: Phase = Phase.DO_TASK,
    model: str | None = "test-model",
    gate_cmd: str | None = None,
    context: str | None = None,
    timeout: int = 60,
) -> Dispatch:
    return Dispatch(
        workflow_id="wf-test-1-1000",
        phase=phase,
        project="test",
        spec_folder="tanren/specs/test",
        branch="main",
        cli=cli,
        model=model,
        gate_cmd=gate_cmd,
        context=context,
        timeout=timeout,
    )


def _setup_command_file(tmp_path: Path, config: Config, phase: Phase) -> Path:
    """Create the command file that _spawn_opencode/codex/claude reads."""
    cmd_dir = tmp_path / config.commands_dir
    cmd_dir.mkdir(parents=True, exist_ok=True)
    cmd_file = cmd_dir / f"{phase.value}.md"
    cmd_file.write_text("# Test command template\nDo the thing.")
    return cmd_file


# ---------------------------------------------------------------------------
# assemble_prompt tests
# ---------------------------------------------------------------------------


class TestAssemblePrompt:
    def test_with_all_fields(self, tmp_path: Path) -> None:
        cmd_file = tmp_path / "do-task.md"
        cmd_file.write_text("# Template\nInstruction body.")

        prompt = assemble_prompt(
            command_file=cmd_file,
            spec_folder="tanren/specs/my-spec",
            command_name="do-task",
            context="Extra context about the task.",
        )

        assert "# Template" in prompt
        assert "Instruction body." in prompt
        assert "Spec folder: tanren/specs/my-spec" in prompt
        assert "Extra context about the task." in prompt
        assert "tanren/specs/my-spec/.agent-status" in prompt
        assert "do-task-status: complete" in prompt

    def test_minimal(self, tmp_path: Path) -> None:
        cmd_file = tmp_path / "audit-task.md"
        cmd_file.write_text("Audit instructions.")

        prompt = assemble_prompt(
            command_file=cmd_file,
            spec_folder="specs/s1",
            command_name="audit-task",
            context=None,
        )

        assert "Audit instructions." in prompt
        assert "Spec folder: specs/s1" in prompt
        assert "specs/s1/.agent-status" in prompt
        # context=None should produce empty extra-context, not the word 'None'
        assert "None" not in prompt


# ---------------------------------------------------------------------------
# spawn_process dispatch routing
# ---------------------------------------------------------------------------


class TestSpawnProcessRouting:
    async def test_routes_to_opencode(self, tmp_path: Path) -> None:
        config = _make_config(tmp_path)
        dispatch = _make_dispatch(Cli.OPENCODE)
        _setup_command_file(tmp_path, config, dispatch.phase)

        with patch("tanren_core.process._spawn_opencode", new_callable=AsyncMock) as mock_spawn:
            mock_spawn.return_value = ProcessResult(exit_code=0, timed_out=False, duration_secs=1)
            result = await spawn_process(dispatch, tmp_path, config)
            mock_spawn.assert_awaited_once_with(dispatch, tmp_path, config, None)
            assert result.exit_code == 0

    async def test_routes_to_codex(self, tmp_path: Path) -> None:
        config = _make_config(tmp_path)
        dispatch = _make_dispatch(Cli.CODEX)
        _setup_command_file(tmp_path, config, dispatch.phase)

        with patch("tanren_core.process._spawn_codex", new_callable=AsyncMock) as mock_spawn:
            mock_spawn.return_value = ProcessResult(exit_code=0, timed_out=False, duration_secs=1)
            result = await spawn_process(dispatch, tmp_path, config)
            mock_spawn.assert_awaited_once_with(dispatch, tmp_path, config, None)
            assert result.exit_code == 0

    async def test_routes_to_claude(self, tmp_path: Path) -> None:
        config = _make_config(tmp_path)
        dispatch = _make_dispatch(Cli.CLAUDE)
        _setup_command_file(tmp_path, config, dispatch.phase)

        with patch("tanren_core.process._spawn_claude", new_callable=AsyncMock) as mock_spawn:
            mock_spawn.return_value = ProcessResult(exit_code=0, timed_out=False, duration_secs=1)
            result = await spawn_process(dispatch, tmp_path, config)
            mock_spawn.assert_awaited_once_with(dispatch, tmp_path, config, None)
            assert result.exit_code == 0

    async def test_routes_to_bash(self, tmp_path: Path) -> None:
        config = _make_config(tmp_path)
        dispatch = _make_dispatch(Cli.BASH, gate_cmd="echo ok")

        with patch("tanren_core.process._spawn_bash", new_callable=AsyncMock) as mock_spawn:
            mock_spawn.return_value = ProcessResult(exit_code=0, timed_out=False, duration_secs=1)
            result = await spawn_process(dispatch, tmp_path, config)
            mock_spawn.assert_awaited_once_with(dispatch, tmp_path, None)
            assert result.exit_code == 0


# ---------------------------------------------------------------------------
# _spawn_opencode — verify constructed command
# ---------------------------------------------------------------------------


class TestSpawnOpencode:
    async def test_command_args(self, tmp_path: Path) -> None:
        config = _make_config(tmp_path)
        dispatch = _make_dispatch(Cli.OPENCODE, model="provider/gpt-4")
        _setup_command_file(tmp_path, config, dispatch.phase)

        with patch("tanren_core.process._run_with_timeout", new_callable=AsyncMock) as mock_run:
            mock_run.return_value = ProcessResult(exit_code=0, timed_out=False, duration_secs=2)
            await _spawn_opencode(dispatch, tmp_path, config)

            mock_run.assert_awaited_once()
            call_args = mock_run.call_args
            cmd = call_args[1]["cmd"] if "cmd" in call_args[1] else call_args[0][0]

            assert cmd[0] == "/usr/local/bin/opencode"
            assert cmd[1] == "run"
            assert "--model" in cmd
            model_idx = cmd.index("--model")
            assert cmd[model_idx + 1] == "provider/gpt-4"
            assert "--dir" in cmd
            dir_idx = cmd.index("--dir")
            assert cmd[dir_idx + 1] == str(tmp_path)
            assert "-f" in cmd

    async def test_no_model(self, tmp_path: Path) -> None:
        config = _make_config(tmp_path)
        dispatch = _make_dispatch(Cli.OPENCODE, model=None)
        _setup_command_file(tmp_path, config, dispatch.phase)

        with patch("tanren_core.process._run_with_timeout", new_callable=AsyncMock) as mock_run:
            mock_run.return_value = ProcessResult(exit_code=0, timed_out=False, duration_secs=1)
            await _spawn_opencode(dispatch, tmp_path, config)

            cmd = mock_run.call_args[0][0]
            assert "--model" not in cmd


# ---------------------------------------------------------------------------
# _spawn_codex — verify constructed command
# ---------------------------------------------------------------------------


class TestSpawnCodex:
    async def test_command_args(self, tmp_path: Path) -> None:
        config = _make_config(tmp_path)
        dispatch = _make_dispatch(Cli.CODEX, model="o3")
        _setup_command_file(tmp_path, config, dispatch.phase)

        with patch("tanren_core.process._run_with_timeout", new_callable=AsyncMock) as mock_run:
            mock_run.return_value = ProcessResult(exit_code=0, timed_out=False, duration_secs=2)
            await _spawn_codex(dispatch, tmp_path, config)

            mock_run.assert_awaited_once()
            cmd = mock_run.call_args[0][0]

            assert cmd[0] == "/usr/local/bin/codex"
            assert "exec" in cmd
            assert "--dangerously-bypass-approvals-and-sandbox" in cmd
            assert "--model" in cmd
            model_idx = cmd.index("--model")
            assert cmd[model_idx + 1] == "o3"
            assert "-C" in cmd
            assert "-o" in cmd
            # stdin_data should be the prompt
            assert mock_run.call_args[1].get("stdin_data") is not None
            # discard_stdout should be True for codex
            assert mock_run.call_args[1].get("discard_stdout") is True

    async def test_no_model(self, tmp_path: Path) -> None:
        config = _make_config(tmp_path)
        dispatch = _make_dispatch(Cli.CODEX, model=None)
        _setup_command_file(tmp_path, config, dispatch.phase)

        with patch("tanren_core.process._run_with_timeout", new_callable=AsyncMock) as mock_run:
            mock_run.return_value = ProcessResult(exit_code=0, timed_out=False, duration_secs=1)
            await _spawn_codex(dispatch, tmp_path, config)

            cmd = mock_run.call_args[0][0]
            assert "--model" not in cmd


# ---------------------------------------------------------------------------
# _spawn_claude — verify constructed command
# ---------------------------------------------------------------------------


class TestSpawnClaude:
    async def test_command_args(self, tmp_path: Path) -> None:
        config = _make_config(tmp_path)
        dispatch = _make_dispatch(Cli.CLAUDE, model="sonnet")
        _setup_command_file(tmp_path, config, dispatch.phase)

        with patch("tanren_core.process._run_with_timeout", new_callable=AsyncMock) as mock_run:
            mock_run.return_value = ProcessResult(exit_code=0, timed_out=False, duration_secs=3)
            await _spawn_claude(dispatch, tmp_path, config)

            mock_run.assert_awaited_once()
            cmd = mock_run.call_args[0][0]

            assert cmd[0] == "/usr/local/bin/claude"
            assert "-p" in cmd
            assert "--dangerously-skip-permissions" in cmd
            assert "--model" in cmd
            model_idx = cmd.index("--model")
            assert cmd[model_idx + 1] == "sonnet"
            # stdin_data should be the prompt
            assert mock_run.call_args[1].get("stdin_data") is not None

    async def test_no_model(self, tmp_path: Path) -> None:
        config = _make_config(tmp_path)
        dispatch = _make_dispatch(Cli.CLAUDE, model=None)
        _setup_command_file(tmp_path, config, dispatch.phase)

        with patch("tanren_core.process._run_with_timeout", new_callable=AsyncMock) as mock_run:
            mock_run.return_value = ProcessResult(exit_code=0, timed_out=False, duration_secs=1)
            await _spawn_claude(dispatch, tmp_path, config)

            cmd = mock_run.call_args[0][0]
            assert "--model" not in cmd


# ---------------------------------------------------------------------------
# _spawn_bash — verify gate behaviour
# ---------------------------------------------------------------------------


class TestSpawnBash:
    async def test_command_args(self, tmp_path: Path) -> None:
        dispatch = _make_dispatch(Cli.BASH, gate_cmd="pytest -x")

        with patch("tanren_core.process._run_with_timeout", new_callable=AsyncMock) as mock_run:
            mock_run.return_value = ProcessResult(exit_code=0, timed_out=False, duration_secs=1)
            await _spawn_bash(dispatch, tmp_path)

            cmd = mock_run.call_args[0][0]
            assert cmd == ["bash", "-c", "pytest -x"]

    async def test_no_gate_cmd_returns_error(self, tmp_path: Path) -> None:
        dispatch = _make_dispatch(Cli.BASH, gate_cmd=None)
        result = await _spawn_bash(dispatch, tmp_path)
        assert result.exit_code == 1
        assert "No gate_cmd" in result.stdout


# ---------------------------------------------------------------------------
# _run_with_timeout — success and timeout paths
# ---------------------------------------------------------------------------


class TestRunWithTimeoutIntegration:
    async def test_success(self, tmp_path: Path) -> None:
        result = await _run_with_timeout(
            cmd=["echo", "hello world"],
            cwd=tmp_path,
            stdin_data=None,
            timeout=10,
        )
        assert result.exit_code == 0
        assert "hello world" in result.stdout
        assert result.timed_out is False

    @pytest.mark.timeout(15)
    async def test_timeout(self, tmp_path: Path) -> None:
        result = await _run_with_timeout(
            cmd=["sleep", "60"],
            cwd=tmp_path,
            stdin_data=None,
            timeout=1,
        )
        assert result.timed_out is True
        # Process should have been killed
        assert result.exit_code != 0 or result.exit_code == -1

    async def test_env_merge(self, tmp_path: Path) -> None:
        result = await _run_with_timeout(
            cmd=["bash", "-c", "echo $MY_TEST_VAR"],
            cwd=tmp_path,
            stdin_data=None,
            timeout=10,
            env={"MY_TEST_VAR": "injected_value"},
        )
        assert result.exit_code == 0
        assert "injected_value" in result.stdout

    async def test_discard_stdout(self, tmp_path: Path) -> None:
        result = await _run_with_timeout(
            cmd=["echo", "should be discarded"],
            cwd=tmp_path,
            stdin_data=None,
            timeout=10,
            discard_stdout=True,
        )
        assert result.exit_code == 0
        assert not result.stdout

    async def test_stdin_forwarded(self, tmp_path: Path) -> None:
        result = await _run_with_timeout(
            cmd=["cat"],
            cwd=tmp_path,
            stdin_data="piped input",
            timeout=10,
        )
        assert result.exit_code == 0
        assert "piped input" in result.stdout
