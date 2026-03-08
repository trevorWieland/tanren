"""Tests for process module."""

from pathlib import Path

from worker_manager.process import assemble_prompt


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
