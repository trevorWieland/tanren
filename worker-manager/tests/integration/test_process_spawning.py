"""Integration test: real process spawning and timeout handling."""

from pathlib import Path

import pytest

from worker_manager.config import Config
from worker_manager.process import _run_with_timeout, spawn_process
from worker_manager.schemas import Cli, Dispatch, Phase


class TestRunWithTimeout:
    @pytest.mark.asyncio
    async def test_successful_command(self, tmp_path: Path):
        result = await _run_with_timeout(
            ["echo", "hello"],
            cwd=tmp_path,
            stdin_data=None,
            timeout=10,
        )
        assert result.exit_code == 0
        assert "hello" in result.stdout
        assert not result.timed_out

    @pytest.mark.asyncio
    async def test_failing_command(self, tmp_path: Path):
        result = await _run_with_timeout(
            ["bash", "-c", "exit 42"],
            cwd=tmp_path,
            stdin_data=None,
            timeout=10,
        )
        assert result.exit_code == 42
        assert not result.timed_out

    @pytest.mark.asyncio
    async def test_stdin_data(self, tmp_path: Path):
        result = await _run_with_timeout(
            ["cat"],
            cwd=tmp_path,
            stdin_data="hello from stdin",
            timeout=10,
        )
        assert result.exit_code == 0
        assert "hello from stdin" in result.stdout

    @pytest.mark.asyncio
    @pytest.mark.timeout(15)
    async def test_timeout_kills_process(self, tmp_path: Path):
        result = await _run_with_timeout(
            ["sleep", "60"],
            cwd=tmp_path,
            stdin_data=None,
            timeout=1,
        )
        assert result.timed_out
        assert result.duration_secs >= 1

    @pytest.mark.asyncio
    async def test_captures_stderr_in_stdout(self, tmp_path: Path):
        result = await _run_with_timeout(
            ["bash", "-c", "echo error >&2"],
            cwd=tmp_path,
            stdin_data=None,
            timeout=10,
        )
        assert "error" in result.stdout


class TestSpawnBashGate:
    @pytest.mark.asyncio
    async def test_gate_success(self, tmp_path: Path):
        config = Config(
            ipc_dir=str(tmp_path),
            github_dir=str(tmp_path),
            data_dir=str(tmp_path),
            worktree_registry_path=str(tmp_path / "worktrees.json"),
        )
        dispatch = Dispatch(
            workflow_id="wf-test-1-1000",
            phase=Phase.GATE,
            project="test",
            spec_folder="tanren/specs/test",
            branch="main",
            cli=Cli.BASH,
            model=None,
            gate_cmd="echo 'all tests passed'",
            context=None,
            timeout=10,
        )
        result = await spawn_process(dispatch, tmp_path, config)
        assert result.exit_code == 0
        assert "all tests passed" in result.stdout

    @pytest.mark.asyncio
    async def test_gate_failure(self, tmp_path: Path):
        config = Config(
            ipc_dir=str(tmp_path),
            github_dir=str(tmp_path),
            data_dir=str(tmp_path),
            worktree_registry_path=str(tmp_path / "worktrees.json"),
        )
        dispatch = Dispatch(
            workflow_id="wf-test-1-1000",
            phase=Phase.GATE,
            project="test",
            spec_folder="tanren/specs/test",
            branch="main",
            cli=Cli.BASH,
            model=None,
            gate_cmd="exit 1",
            context=None,
            timeout=10,
        )
        result = await spawn_process(dispatch, tmp_path, config)
        assert result.exit_code == 1

    @pytest.mark.asyncio
    async def test_no_gate_cmd(self, tmp_path: Path):
        config = Config(
            ipc_dir=str(tmp_path),
            github_dir=str(tmp_path),
            data_dir=str(tmp_path),
            worktree_registry_path=str(tmp_path / "worktrees.json"),
        )
        dispatch = Dispatch(
            workflow_id="wf-test-1-1000",
            phase=Phase.GATE,
            project="test",
            spec_folder="tanren/specs/test",
            branch="main",
            cli=Cli.BASH,
            model=None,
            gate_cmd=None,
            context=None,
            timeout=10,
        )
        result = await spawn_process(dispatch, tmp_path, config)
        assert result.exit_code == 1


class TestCliArgSmoke:
    """Smoke tests that invoke real CLI binaries to verify arg ordering.

    These use a nonexistent model so the CLI exits quickly without
    doing real work, but they catch arg-ordering regressions that
    cause silent failures in production.
    """

    @pytest.mark.asyncio
    @pytest.mark.timeout(15)
    async def test_opencode_accepts_correct_arg_order(self, tmp_path: Path):
        """Correct arg order: opencode run --model X --dir Y "msg" -f file."""
        prompt_file = tmp_path / "prompt.md"
        prompt_file.write_text("Test prompt — ignore this.")

        cmd = [
            "opencode",
            "run",
            "--model",
            "nonexistent/model",
            "--dir",
            str(tmp_path),
            "Read the attached file and follow its instructions exactly.",
            "-f",
            str(prompt_file),
        ]

        result = await _run_with_timeout(
            cmd,
            cwd=tmp_path,
            stdin_data=None,
            timeout=10,
        )
        # "File not found" in stdout means opencode interpreted -f as part of
        # the message rather than as a flag — i.e. arg ordering is wrong.
        assert "File not found" not in result.stdout

    @pytest.mark.asyncio
    @pytest.mark.timeout(15)
    async def test_opencode_rejects_wrong_arg_order(self, tmp_path: Path):
        """Wrong arg order: -f before positional triggers 'File not found'."""
        prompt_file = tmp_path / "prompt.md"
        prompt_file.write_text("Test prompt — ignore this.")

        cmd = [
            "opencode",
            "run",
            "--model",
            "nonexistent/model",
            "--dir",
            str(tmp_path),
            "-f",
            str(prompt_file),
            "Read the attached file and follow its instructions exactly.",
        ]

        result = await _run_with_timeout(
            cmd,
            cwd=tmp_path,
            stdin_data=None,
            timeout=10,
        )
        # With wrong order, we expect an error about the file or arg parsing.
        # This validates our test oracle — if this test fails, the oracle
        # assumption in the test above is invalid.
        assert "File not found" in result.stdout or result.exit_code != 0


class TestCodexCliArgSmoke:
    @pytest.mark.asyncio
    @pytest.mark.timeout(15)
    async def test_codex_accepts_correct_arg_order(self, tmp_path: Path):
        """Correct arg order passes clap arg parsing (exit_code != 2)."""
        cmd = [
            "codex",
            "exec",
            "--dangerously-bypass-approvals-and-sandbox",
            "--model",
            "nonexistent-model",
            "-C",
            str(tmp_path),
            "--skip-git-repo-check",
        ]

        result = await _run_with_timeout(
            cmd,
            cwd=tmp_path,
            stdin_data="Test prompt",
            timeout=10,
        )
        # exit_code 2 = clap arg-parse error
        assert result.exit_code != 2
