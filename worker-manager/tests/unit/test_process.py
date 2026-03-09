"""Tests for process module."""

import asyncio
from pathlib import Path
from unittest.mock import AsyncMock, patch

from worker_manager.process import ProcessResult, _spawn_opencode, assemble_prompt


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
    def test_uses_stdin_not_file_flag(self, tmp_path: Path):
        """opencode receives prompt via stdin, not -f flag."""
        commands_dir = tmp_path / ".claude" / "commands" / "tanren"
        commands_dir.mkdir(parents=True)
        (commands_dir / "do-task.md").write_text("# Do Task")

        from worker_manager.config import Config
        from worker_manager.schemas import Cli, Dispatch, Phase

        dispatch = Dispatch(
            workflow_id="wf-test-1-1234567890",
            phase=Phase.DO_TASK,
            project="test",
            spec_folder="tanren/specs/test",
            branch="test-branch",
            cli=Cli.OPENCODE,
            model="zai-coding-plan/glm-5",
            gate_cmd=None,
            context=None,
            timeout=1800,
        )

        config = Config(
            ipc_dir=str(tmp_path / "ipc"),
            github_dir=str(tmp_path),
            commands_dir=".claude/commands/tanren",
            data_dir=str(tmp_path / "data"),
            worktree_registry_path=str(tmp_path / "worktrees.json"),
        )

        mock_result = ProcessResult(
            exit_code=0, stdout="done", timed_out=False, duration_secs=10
        )

        with patch(
            "worker_manager.process._run_with_timeout",
            new_callable=AsyncMock,
            return_value=mock_result,
        ) as mock_run:
            result = asyncio.run(
                _spawn_opencode(dispatch, tmp_path, config)
            )

            mock_run.assert_called_once()
            call_kwargs = mock_run.call_args
            cmd = call_kwargs[0][0] if call_kwargs[0] else call_kwargs[1]["cmd"]

            # Verify -f flag is NOT in the command
            assert "-f" not in cmd

            # Verify stdin_data contains the prompt
            if call_kwargs[1]:
                assert call_kwargs[1].get("stdin_data") is not None
            else:
                # positional: cmd, cwd, stdin_data, timeout
                assert call_kwargs[0][2] is not None

            # Verify model is passed
            assert "--model" in cmd
            assert "zai-coding-plan/glm-5" in cmd

        assert result.exit_code == 0

    def test_prompt_content_passed_via_stdin(self, tmp_path: Path):
        """Assembled prompt content is passed as stdin_data."""
        commands_dir = tmp_path / ".claude" / "commands" / "tanren"
        commands_dir.mkdir(parents=True)
        (commands_dir / "do-task.md").write_text("# Do Task\n\nImplement it.")

        from worker_manager.config import Config
        from worker_manager.schemas import Cli, Dispatch, Phase

        dispatch = Dispatch(
            workflow_id="wf-test-1-1234567890",
            phase=Phase.DO_TASK,
            project="test",
            spec_folder="tanren/specs/test",
            branch="test-branch",
            cli=Cli.OPENCODE,
            model=None,
            gate_cmd=None,
            context="Extra context here",
            timeout=1800,
        )

        config = Config(
            ipc_dir=str(tmp_path / "ipc"),
            github_dir=str(tmp_path),
            commands_dir=".claude/commands/tanren",
            data_dir=str(tmp_path / "data"),
            worktree_registry_path=str(tmp_path / "worktrees.json"),
        )

        mock_result = ProcessResult(
            exit_code=0, stdout="", timed_out=False, duration_secs=1
        )

        with patch(
            "worker_manager.process._run_with_timeout",
            new_callable=AsyncMock,
            return_value=mock_result,
        ) as mock_run:
            asyncio.run(
                _spawn_opencode(dispatch, tmp_path, config)
            )

            call_args = mock_run.call_args
            # Extract stdin_data from kwargs or positional args
            if "stdin_data" in call_args.kwargs:
                stdin_data = call_args.kwargs["stdin_data"]
            else:
                stdin_data = call_args.args[2]

            assert "# Do Task" in stdin_data
            assert "Implement it." in stdin_data
            assert "Extra context here" in stdin_data
            assert "tanren/specs/test/.agent-status" in stdin_data
