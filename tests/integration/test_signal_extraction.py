"""Integration tests for remote signal extraction helper."""

from tanren_core.adapters.ssh_environment import _extract_signal_token


class TestExtractSignalTokenIntegration:
    """Verify _extract_signal_token covers all dispatch phase signal patterns."""

    def test_do_task_signals(self):
        for signal in ("complete", "blocked", "all-done", "error"):
            token = _extract_signal_token("do-task", f"do-task-status: {signal}", "")
            assert token == signal

    def test_audit_task_signals(self):
        for signal in ("pass", "fail", "error"):
            token = _extract_signal_token("audit-task", f"audit-task-status: {signal}", "")
            assert token == signal

    def test_run_demo_signals(self):
        for signal in ("pass", "fail", "error"):
            token = _extract_signal_token("run-demo", f"run-demo-status: {signal}", "")
            assert token == signal

    def test_stdout_fallback_all_phases(self):
        for cmd in ("do-task", "audit-task", "run-demo"):
            token = _extract_signal_token(cmd, "", f"output\n{cmd}-status: complete\n")
            assert token == "complete"

    def test_file_precedence_over_stdout(self):
        token = _extract_signal_token(
            "do-task", "do-task-status: blocked", "do-task-status: complete"
        )
        assert token == "blocked"
