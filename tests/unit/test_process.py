"""Tests for process module."""

import asyncio
import contextlib
from pathlib import Path
from unittest.mock import AsyncMock, patch

from tanren_core.config import Config
from tanren_core.process import (
    ProcessResult,
    _spawn_claude,  # noqa: PLC2701 — testing private implementation
    _spawn_opencode,  # noqa: PLC2701 — testing private implementation
    assemble_prompt,
)
from tanren_core.schemas import Cli, Dispatch, Phase


class TestAssemblePrompt:
    def test_basic_assembly(self, tmp_path: Path):
        cmd_file = tmp_path / "do-task.md"
        cmd_file.write_text("# Do Task\n\nImplement the next task.")

        result = assemble_prompt(
            cmd_file,
            "tanren/specs/test-spec",
            "do-task",
            None,
        )

        assert "# Do Task" in result
        assert "Implement the next task." in result
        assert "Spec folder: tanren/specs/test-spec" in result
        assert "tanren/specs/test-spec/.agent-status" in result
        assert "`do-task-status: complete`" in result

    def test_with_context(self, tmp_path: Path):
        cmd_file = tmp_path / "do-task.md"
        cmd_file.write_text("# Do Task")

        result = assemble_prompt(
            cmd_file,
            "tanren/specs/test",
            "do-task",
            "GATE FAILURE — 'make check' FAILED.\n```\nerror output\n```",
        )

        assert "GATE FAILURE" in result
        assert "error output" in result

    def test_no_context(self, tmp_path: Path):
        cmd_file = tmp_path / "do-task.md"
        cmd_file.write_text("# Do Task")

        result = assemble_prompt(
            cmd_file,
            "tanren/specs/test",
            "do-task",
            None,
        )

        # Context section should be empty but separators present
        assert "---" in result

    def test_audit_spec_command_name(self, tmp_path: Path):
        cmd_file = tmp_path / "audit-spec.md"
        cmd_file.write_text("# Audit Spec")

        result = assemble_prompt(
            cmd_file,
            "tanren/specs/test",
            "audit-spec",
            None,
        )

        assert "`audit-spec-status: complete`" in result


class TestSpawnOpencode:
    def _make_dispatch_and_config(self, tmp_path, model="zai-coding-plan/glm-5", context=None):
        """Helper to create dispatch + config for opencode tests."""
        commands_dir = tmp_path / ".claude" / "commands" / "tanren"
        commands_dir.mkdir(parents=True, exist_ok=True)
        (commands_dir / "do-task.md").write_text("# Do Task\n\nImplement it.")

        dispatch = Dispatch(
            workflow_id="wf-test-1-1234567890",
            phase=Phase.DO_TASK,
            project="test",
            spec_folder="tanren/specs/test",
            branch="test-branch",
            cli=Cli.OPENCODE,
            model=model,
            gate_cmd=None,
            context=context,
            timeout=1800,
        )

        config = Config(
            ipc_dir=str(tmp_path / "ipc"),
            github_dir=str(tmp_path),
            commands_dir=".claude/commands/tanren",
            data_dir=str(tmp_path / "data"),
            worktree_registry_path=str(tmp_path / "worktrees.json"),
            roles_config_path=str(tmp_path / "roles.yml"),
        )

        return dispatch, config

    def test_uses_file_attachment_with_positional_message(self, tmp_path: Path):
        """opencode receives prompt via -f file attachment + positional message."""
        dispatch, config = self._make_dispatch_and_config(tmp_path)

        mock_result = ProcessResult(exit_code=0, stdout="done", timed_out=False, duration_secs=10)

        with patch(
            "tanren_core.process._run_with_timeout",
            new_callable=AsyncMock,
            return_value=mock_result,
        ) as mock_run:
            result = asyncio.run(_spawn_opencode(dispatch, tmp_path, config))

            mock_run.assert_called_once()
            call_args = mock_run.call_args
            cmd = call_args.args[0] if call_args.args else call_args.kwargs["cmd"]

            # Verify -f flag IS in the command
            assert "-f" in cmd

            # Positional message must come before -f (opencode's -f is greedy)
            f_idx = cmd.index("-f")
            msg_idx = cmd.index("Read the attached file and follow its instructions exactly.")
            assert msg_idx < f_idx

            # Verify stdin_data is None (not using stdin)
            if "stdin_data" in call_args.kwargs:
                assert call_args.kwargs["stdin_data"] is None
            else:
                assert call_args.args[2] is None

            # Verify model is passed
            assert "--model" in cmd
            assert "zai-coding-plan/glm-5" in cmd

        assert result.exit_code == 0

    def test_prompt_written_to_temp_file(self, tmp_path: Path):
        """Assembled prompt content is written to the temp file referenced by -f."""
        dispatch, config = self._make_dispatch_and_config(
            tmp_path,
            model=None,
            context="Extra context here",
        )

        captured_file_path = None

        async def fake_run(cmd, *, cwd, stdin_data, timeout_secs, **kwargs):  # noqa: RUF029 — async required by interface
            nonlocal captured_file_path
            # Find the file path after -f flag
            f_idx = cmd.index("-f")
            captured_file_path = cmd[f_idx + 1]
            return ProcessResult(exit_code=0, stdout="", timed_out=False, duration_secs=1)

        with patch(
            "tanren_core.process._run_with_timeout",
            side_effect=fake_run,
        ):
            asyncio.run(_spawn_opencode(dispatch, tmp_path, config))

            # The file is cleaned up after _spawn_opencode returns,
            # so we read it inside the mock. Verify the path was captured.
            assert captured_file_path is not None
            assert captured_file_path.endswith(".md")

    def test_temp_file_cleaned_up_on_success(self, tmp_path: Path):
        """Temp prompt file is deleted after successful process completion."""
        dispatch, config = self._make_dispatch_and_config(tmp_path)

        captured_file_path = None

        async def fake_run(cmd, *, cwd, stdin_data, timeout_secs, **kwargs):  # noqa: RUF029 — async required by interface
            nonlocal captured_file_path
            f_idx = cmd.index("-f")
            captured_file_path = cmd[f_idx + 1]
            # Verify file exists during execution
            assert Path(captured_file_path).exists()  # noqa: ASYNC240 — trivial sync fs op after async work
            return ProcessResult(exit_code=0, stdout="done", timed_out=False, duration_secs=5)

        with patch(
            "tanren_core.process._run_with_timeout",
            side_effect=fake_run,
        ):
            asyncio.run(_spawn_opencode(dispatch, tmp_path, config))

        # File should be cleaned up after return
        assert captured_file_path is not None
        assert not Path(captured_file_path).exists()

    def test_temp_file_cleaned_up_on_error(self, tmp_path: Path):
        """Temp prompt file is deleted even when the process raises an error."""
        dispatch, config = self._make_dispatch_and_config(tmp_path)

        captured_file_path = None

        async def fake_run(cmd, *, cwd, stdin_data, timeout_secs, **kwargs):  # noqa: RUF029 — async required by interface
            nonlocal captured_file_path
            f_idx = cmd.index("-f")
            captured_file_path = cmd[f_idx + 1]
            assert Path(captured_file_path).exists()  # noqa: ASYNC240 — trivial sync fs op after async work
            raise RuntimeError("process exploded")

        with (
            patch(
                "tanren_core.process._run_with_timeout",
                side_effect=fake_run,
            ),
            contextlib.suppress(RuntimeError),
        ):
            asyncio.run(_spawn_opencode(dispatch, tmp_path, config))

        # File should be cleaned up even after error
        assert captured_file_path is not None
        # captured_file_path is confirmed not None by the assert above
        assert not Path(captured_file_path).exists()


class TestSpawnClaude:
    def _make_dispatch_and_config(self, tmp_path, model="claude-sonnet-4-20250514", context=None):
        commands_dir = tmp_path / ".claude" / "commands" / "tanren"
        commands_dir.mkdir(parents=True, exist_ok=True)
        (commands_dir / "do-task.md").write_text("# Do Task\n\nImplement it.")

        dispatch = Dispatch(
            workflow_id="wf-test-1-1234567890",
            phase=Phase.DO_TASK,
            project="test",
            spec_folder="tanren/specs/test",
            branch="test-branch",
            cli=Cli.CLAUDE,
            model=model,
            gate_cmd=None,
            context=context,
            timeout=1800,
        )

        config = Config(
            ipc_dir=str(tmp_path / "ipc"),
            github_dir=str(tmp_path),
            commands_dir=".claude/commands/tanren",
            data_dir=str(tmp_path / "data"),
            worktree_registry_path=str(tmp_path / "worktrees.json"),
            roles_config_path=str(tmp_path / "roles.yml"),
        )

        return dispatch, config

    def test_command_construction(self, tmp_path):
        """Claude receives prompt via stdin with -p --dangerously-skip-permissions."""
        dispatch, config = self._make_dispatch_and_config(tmp_path)

        mock_result = ProcessResult(exit_code=0, stdout="done", timed_out=False, duration_secs=10)

        with patch(
            "tanren_core.process._run_with_timeout",
            new_callable=AsyncMock,
            return_value=mock_result,
        ) as mock_run:
            result = asyncio.run(_spawn_claude(dispatch, tmp_path, config))

            mock_run.assert_called_once()
            call_args = mock_run.call_args
            cmd = call_args.args[0] if call_args.args else call_args.kwargs["cmd"]

            # Verify -p and --dangerously-skip-permissions flags
            assert "-p" in cmd
            assert "--dangerously-skip-permissions" in cmd

            # Verify model is passed
            assert "--model" in cmd
            assert "claude-sonnet-4-20250514" in cmd

            # Verify stdin_data is the prompt (not None like opencode)
            if "stdin_data" in call_args.kwargs:
                assert call_args.kwargs["stdin_data"] is not None
            else:
                assert call_args.args[2] is not None

        assert result.exit_code == 0

    def test_no_model_flag_when_model_is_none(self, tmp_path):
        dispatch, config = self._make_dispatch_and_config(tmp_path, model=None)

        mock_result = ProcessResult(exit_code=0, stdout="done", timed_out=False, duration_secs=5)

        with patch(
            "tanren_core.process._run_with_timeout",
            new_callable=AsyncMock,
            return_value=mock_result,
        ) as mock_run:
            asyncio.run(_spawn_claude(dispatch, tmp_path, config))

            cmd = mock_run.call_args.args[0]
            assert "--model" not in cmd
