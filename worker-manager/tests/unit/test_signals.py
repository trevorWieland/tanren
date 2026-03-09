"""Tests for signals module — covers every outcome mapping row from PROTOCOL.md."""

from pathlib import Path

from worker_manager.schemas import FindingSeverity, Outcome, Phase
from worker_manager.signals import (
    extract_signal,
    map_outcome,
    parse_audit_findings,
    parse_audit_spec_findings,
    parse_demo_findings,
    parse_investigation_report,
)


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


class TestParseAuditFindings:
    def test_valid_findings(self, tmp_path: Path):
        (tmp_path / "audit-findings.json").write_text(
            '{"signal": "fail", "findings": [{"title": "Bug", "severity": "fix"}]}'
        )
        result = parse_audit_findings(tmp_path)
        assert result is not None
        assert result.signal == "fail"
        assert len(result.findings) == 1
        assert result.findings[0].title == "Bug"

    def test_pass_no_findings(self, tmp_path: Path):
        (tmp_path / "audit-findings.json").write_text(
            '{"signal": "pass", "findings": []}'
        )
        result = parse_audit_findings(tmp_path)
        assert result is not None
        assert result.signal == "pass"
        assert result.findings == []

    def test_missing_file(self, tmp_path: Path):
        assert parse_audit_findings(tmp_path) is None

    def test_malformed_json(self, tmp_path: Path):
        (tmp_path / "audit-findings.json").write_text("not json")
        assert parse_audit_findings(tmp_path) is None

    def test_best_effort_code_fences(self, tmp_path: Path):
        (tmp_path / "audit-findings.json").write_text(
            '```json\n{"signal": "fail", "findings": [{"title": "Bug"}]}\n```'
        )
        result = parse_audit_findings(tmp_path)
        assert result is not None
        assert result.signal == "fail"


class TestParseDemoFindings:
    def test_valid(self, tmp_path: Path):
        (tmp_path / "demo-findings.json").write_text(
            '{"signal": "fail", "findings": [{"title": "Step 2 failed", "severity": "fix"}]}'
        )
        result = parse_demo_findings(tmp_path)
        assert result is not None
        assert len(result.findings) == 1

    def test_missing(self, tmp_path: Path):
        assert parse_demo_findings(tmp_path) is None


class TestParseAuditSpecFindings:
    def test_with_markers(self, tmp_path: Path):
        (tmp_path / "audit.md").write_text(
            'status: fail\n\n'
            '<!-- structured-findings-start -->\n'
            '[{"title": "Missing tests", "severity": "fix"}]\n'
            '<!-- structured-findings-end -->\n\n'
            '# Audit details...'
        )
        result = parse_audit_spec_findings(tmp_path)
        assert len(result) == 1
        assert result[0].title == "Missing tests"
        assert result[0].severity == FindingSeverity.FIX

    def test_no_markers(self, tmp_path: Path):
        (tmp_path / "audit.md").write_text("status: fail\n\n# Audit details")
        result = parse_audit_spec_findings(tmp_path)
        assert result == []

    def test_missing_file(self, tmp_path: Path):
        result = parse_audit_spec_findings(tmp_path)
        assert result == []

    def test_malformed_json_in_markers(self, tmp_path: Path):
        (tmp_path / "audit.md").write_text(
            '<!-- structured-findings-start -->\n'
            'not json\n'
            '<!-- structured-findings-end -->'
        )
        result = parse_audit_spec_findings(tmp_path)
        assert result == []


class TestParseInvestigationReport:
    def test_valid(self, tmp_path: Path):
        (tmp_path / "investigation-report.json").write_text(
            '{"trigger": "gate_failure_persistent", "root_causes": ['
            '{"description": "Wrong call", "confidence": "high", '
            '"category": "code_bug", "suggested_tasks": [{"title": "Fix it"}]}'
            ']}'
        )
        result = parse_investigation_report(tmp_path)
        assert result is not None
        assert result.trigger == "gate_failure_persistent"
        assert len(result.root_causes) == 1
        assert result.root_causes[0].confidence == "high"

    def test_missing(self, tmp_path: Path):
        assert parse_investigation_report(tmp_path) is None

    def test_malformed(self, tmp_path: Path):
        (tmp_path / "investigation-report.json").write_text("bad json")
        assert parse_investigation_report(tmp_path) is None

    def test_with_escalation(self, tmp_path: Path):
        (tmp_path / "investigation-report.json").write_text(
            '{"trigger": "demo_failure", "escalation_needed": true, '
            '"escalation_reason": "Spec unclear"}'
        )
        result = parse_investigation_report(tmp_path)
        assert result is not None
        assert result.escalation_needed is True
