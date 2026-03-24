"""Integration tests for remote signal extraction and auth validation."""

import pytest

from tanren_core.adapters.ssh_environment import _extract_signal_token, _validate_cli_auth
from tanren_core.schemas import Cli


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


class TestValidateCliAuthIntegration:
    """Verify CLI auth validation for all supported CLIs."""

    def test_claude_requires_at_least_one_auth(self):
        _validate_cli_auth(Cli.CLAUDE, {"CLAUDE_CODE_OAUTH_TOKEN": "tok"})
        _validate_cli_auth(Cli.CLAUDE, {"CLAUDE_CREDENTIALS_JSON": "{}"})
        with pytest.raises(RuntimeError):
            _validate_cli_auth(Cli.CLAUDE, {})

    def test_opencode_requires_api_key(self):
        _validate_cli_auth(Cli.OPENCODE, {"OPENCODE_ZAI_API_KEY": "key"})
        with pytest.raises(RuntimeError):
            _validate_cli_auth(Cli.OPENCODE, {})

    def test_codex_requires_auth_json(self):
        _validate_cli_auth(Cli.CODEX, {"CODEX_AUTH_JSON": "{}"})
        with pytest.raises(RuntimeError):
            _validate_cli_auth(Cli.CODEX, {})

    def test_bash_needs_no_auth(self):
        _validate_cli_auth(Cli.BASH, {})
