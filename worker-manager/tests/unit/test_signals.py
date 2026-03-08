"""Tests for signals module — covers every outcome mapping row from PROTOCOL.md."""

from pathlib import Path

from worker_manager.schemas import Outcome, Phase
from worker_manager.signals import extract_signal, map_outcome


class TestExtractSignal:
    def test_from_agent_status_file(self, tmp_path: Path):
        (tmp_path / ".agent-status").write_text("do-task-status: complete\n")
        result = extract_signal(Phase.DO_TASK, "do-task", tmp_path, "")
        assert result == "complete"

    def test_fallback_to_stdout(self, tmp_path: Path):
        stdout = "lots of output\ndo-task-status: blocked\nmore output"
        result = extract_signal(Phase.DO_TASK, "do-task", tmp_path, stdout)
        assert result == "blocked"

    def test_file_takes_precedence_over_stdout(self, tmp_path: Path):
        (tmp_path / ".agent-status").write_text("do-task-status: complete\n")
        stdout = "do-task-status: error"
        result = extract_signal(Phase.DO_TASK, "do-task", tmp_path, stdout)
        assert result == "complete"

    def test_audit_task_signal(self, tmp_path: Path):
        (tmp_path / ".agent-status").write_text("audit-task-status: pass\n")
        result = extract_signal(Phase.AUDIT_TASK, "audit-task", tmp_path, "")
        assert result == "pass"

    def test_audit_spec_from_audit_md(self, tmp_path: Path):
        (tmp_path / "audit.md").write_text("status: pass\n\nSome audit content")
        result = extract_signal(Phase.AUDIT_SPEC, "audit-spec", tmp_path, "")
        assert result == "pass"

    def test_audit_spec_fail(self, tmp_path: Path):
        (tmp_path / "audit.md").write_text("status: fail\n\nIssues found")
        result = extract_signal(Phase.AUDIT_SPEC, "audit-spec", tmp_path, "")
        assert result == "fail"

    def test_audit_spec_unknown_returns_none(self, tmp_path: Path):
        (tmp_path / "audit.md").write_text("status: unknown\n")
        result = extract_signal(Phase.AUDIT_SPEC, "audit-spec", tmp_path, "")
        assert result is None

    def test_audit_spec_missing_audit_md(self, tmp_path: Path):
        result = extract_signal(Phase.AUDIT_SPEC, "audit-spec", tmp_path, "")
        assert result is None

    def test_gate_returns_none(self, tmp_path: Path):
        result = extract_signal(Phase.GATE, "gate", tmp_path, "output")
        assert result is None

    def test_setup_returns_none(self, tmp_path: Path):
        result = extract_signal(Phase.SETUP, "setup", tmp_path, "")
        assert result is None

    def test_cleanup_returns_none(self, tmp_path: Path):
        result = extract_signal(Phase.CLEANUP, "cleanup", tmp_path, "")
        assert result is None

    def test_no_signal_found(self, tmp_path: Path):
        result = extract_signal(Phase.DO_TASK, "do-task", tmp_path, "no signal here")
        assert result is None

    def test_last_signal_wins_in_stdout(self, tmp_path: Path):
        stdout = "do-task-status: blocked\ndo-task-status: complete"
        result = extract_signal(Phase.DO_TASK, "do-task", tmp_path, stdout)
        assert result == "complete"

    def test_run_demo_signal(self, tmp_path: Path):
        (tmp_path / ".agent-status").write_text("run-demo-status: pass\n")
        result = extract_signal(Phase.RUN_DEMO, "run-demo", tmp_path, "")
        assert result == "pass"


class TestMapOutcome:
    """Tests every row of the outcome mapping table from PROTOCOL.md Section 3."""

    def test_complete_maps_to_success(self):
        assert map_outcome(Phase.DO_TASK, "complete", 0, False) == (Outcome.SUCCESS, "complete")

    def test_pass_maps_to_success(self):
        assert map_outcome(Phase.AUDIT_TASK, "pass", 0, False) == (Outcome.SUCCESS, "pass")

    def test_all_done_maps_to_success(self):
        assert map_outcome(Phase.DO_TASK, "all-done", 0, False) == (Outcome.SUCCESS, "all-done")

    def test_fail_maps_to_fail(self):
        assert map_outcome(Phase.AUDIT_TASK, "fail", 0, False) == (Outcome.FAIL, "fail")

    def test_blocked_maps_to_blocked(self):
        assert map_outcome(Phase.DO_TASK, "blocked", 0, False) == (Outcome.BLOCKED, "blocked")

    def test_error_maps_to_error(self):
        assert map_outcome(Phase.DO_TASK, "error", 1, False) == (Outcome.ERROR, "error")

    def test_no_signal_exit_0(self):
        assert map_outcome(Phase.DO_TASK, None, 0, False) == (Outcome.SUCCESS, None)

    def test_no_signal_nonzero_exit(self):
        assert map_outcome(Phase.DO_TASK, None, 1, False) == (Outcome.ERROR, None)

    def test_timeout(self):
        assert map_outcome(Phase.DO_TASK, None, -1, True) == (Outcome.TIMEOUT, None)

    def test_timeout_overrides_signal(self):
        assert map_outcome(Phase.DO_TASK, "complete", 0, True) == (Outcome.TIMEOUT, None)

    def test_gate_exit_0(self):
        assert map_outcome(Phase.GATE, None, 0, False) == (Outcome.SUCCESS, None)

    def test_gate_nonzero_exit(self):
        assert map_outcome(Phase.GATE, None, 1, False) == (Outcome.FAIL, None)

    def test_gate_timeout(self):
        assert map_outcome(Phase.GATE, None, -1, True) == (Outcome.TIMEOUT, None)

    def test_setup_success(self):
        assert map_outcome(Phase.SETUP, None, 0, False) == (Outcome.SUCCESS, None)

    def test_setup_error(self):
        assert map_outcome(Phase.SETUP, None, 1, False) == (Outcome.ERROR, None)

    def test_cleanup_success(self):
        assert map_outcome(Phase.CLEANUP, None, 0, False) == (Outcome.SUCCESS, None)

    def test_unrecognized_signal(self):
        outcome, signal = map_outcome(Phase.DO_TASK, "unknown-signal", 0, False)
        assert outcome == Outcome.SUCCESS
        assert signal == "unknown-signal"
