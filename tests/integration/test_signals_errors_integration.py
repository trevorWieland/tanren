"""Integration tests for signal extraction, findings parsing, and error classification."""

import json
from pathlib import Path

from tanren_core.errors import ErrorClass, classify_error
from tanren_core.schemas import Finding, FindingsOutput, InvestigationReport, Outcome, Phase
from tanren_core.signals import (
    extract_signal,
    map_outcome,
    parse_audit_findings,
    parse_audit_spec_findings,
    parse_demo_findings,
    parse_investigation_report,
)

# ---------------------------------------------------------------------------
# extract_signal
# ---------------------------------------------------------------------------


class TestExtractSignal:
    def test_extract_signal_from_status_file(self, tmp_path: Path) -> None:
        (tmp_path / ".agent-status").write_text("do-task-status: complete\n")
        result = extract_signal(Phase.DO_TASK, "do-task", tmp_path, "")
        assert result == "complete"

    def test_extract_signal_stdout_fallback(self, tmp_path: Path) -> None:
        result = extract_signal(Phase.DO_TASK, "do-task", tmp_path, "do-task-status: blocked")
        assert result == "blocked"

    def test_extract_signal_gate_returns_none(self, tmp_path: Path) -> None:
        (tmp_path / ".agent-status").write_text("gate-status: complete\n")
        result = extract_signal(Phase.GATE, "gate", tmp_path, "gate-status: complete")
        assert result is None

    def test_extract_signal_setup_returns_none(self, tmp_path: Path) -> None:
        result = extract_signal(Phase.SETUP, "setup", tmp_path, "setup-status: complete")
        assert result is None

    def test_extract_signal_cleanup_returns_none(self, tmp_path: Path) -> None:
        result = extract_signal(Phase.CLEANUP, "cleanup", tmp_path, "cleanup-status: complete")
        assert result is None

    def test_extract_signal_audit_spec_reads_audit_md(self, tmp_path: Path) -> None:
        (tmp_path / "audit.md").write_text("status: pass\nSome audit content.")
        result = extract_signal(Phase.AUDIT_SPEC, "audit-spec", tmp_path, "")
        assert result == "pass"

    def test_extract_signal_audit_spec_fail(self, tmp_path: Path) -> None:
        (tmp_path / "audit.md").write_text("status: fail\nDetails here.")
        result = extract_signal(Phase.AUDIT_SPEC, "audit-spec", tmp_path, "")
        assert result == "fail"

    def test_extract_signal_audit_spec_unknown_returns_none(self, tmp_path: Path) -> None:
        (tmp_path / "audit.md").write_text("status: unknown\nDetails here.")
        result = extract_signal(Phase.AUDIT_SPEC, "audit-spec", tmp_path, "")
        assert result is None

    def test_extract_signal_audit_spec_missing_file(self, tmp_path: Path) -> None:
        result = extract_signal(Phase.AUDIT_SPEC, "audit-spec", tmp_path, "")
        assert result is None

    def test_extract_signal_no_signal_found(self, tmp_path: Path) -> None:
        result = extract_signal(Phase.DO_TASK, "do-task", tmp_path, "")
        assert result is None


# ---------------------------------------------------------------------------
# Structured findings parsing
# ---------------------------------------------------------------------------


class TestParseFindings:
    def test_parse_audit_findings_valid_json(self, tmp_path: Path) -> None:
        data = {
            "signal": "fail",
            "findings": [
                {"title": "Bug", "severity": "fix", "description": "desc"},
            ],
        }
        (tmp_path / "audit-findings.json").write_text(json.dumps(data))
        result = parse_audit_findings(tmp_path)
        assert isinstance(result, FindingsOutput)
        assert len(result.findings) == 1
        assert result.findings[0].title == "Bug"

    def test_parse_demo_findings_valid_json(self, tmp_path: Path) -> None:
        data = {
            "signal": "complete",
            "findings": [
                {"title": "Demo issue", "severity": "note", "description": "details"},
            ],
        }
        (tmp_path / "demo-findings.json").write_text(json.dumps(data))
        result = parse_demo_findings(tmp_path)
        assert isinstance(result, FindingsOutput)
        assert len(result.findings) == 1
        assert result.findings[0].title == "Demo issue"

    def test_parse_findings_code_fence(self, tmp_path: Path) -> None:
        data = {
            "signal": "fail",
            "findings": [
                {"title": "Fenced bug", "severity": "fix", "description": "wrapped"},
            ],
        }
        content = "```json\n" + json.dumps(data) + "\n```"
        (tmp_path / "audit-findings.json").write_text(content)
        result = parse_audit_findings(tmp_path)
        assert isinstance(result, FindingsOutput)
        assert result.findings[0].title == "Fenced bug"

    def test_parse_findings_missing_file(self, tmp_path: Path) -> None:
        result = parse_audit_findings(tmp_path)
        assert result is None

    def test_parse_findings_invalid_json(self, tmp_path: Path) -> None:
        (tmp_path / "audit-findings.json").write_text("this is not valid json at all")
        result = parse_audit_findings(tmp_path)
        assert result is None


# ---------------------------------------------------------------------------
# Audit spec findings (markers in audit.md)
# ---------------------------------------------------------------------------


class TestParseAuditSpecFindings:
    def test_parse_audit_spec_findings_with_markers(self, tmp_path: Path) -> None:
        findings_json = json.dumps([
            {"title": "Issue A", "severity": "fix", "description": "found a problem"},
            {"title": "Issue B", "severity": "note", "description": "minor note"},
        ])
        content = (
            "status: fail\n"
            "Some prose.\n"
            "<!-- structured-findings-start -->\n"
            f"{findings_json}\n"
            "<!-- structured-findings-end -->\n"
            "More prose.\n"
        )
        (tmp_path / "audit.md").write_text(content)
        result = parse_audit_spec_findings(tmp_path)
        assert len(result) == 2
        assert all(isinstance(f, Finding) for f in result)
        assert result[0].title == "Issue A"
        assert result[1].title == "Issue B"

    def test_parse_audit_spec_findings_no_markers(self, tmp_path: Path) -> None:
        (tmp_path / "audit.md").write_text("status: pass\nJust plain content.\n")
        result = parse_audit_spec_findings(tmp_path)
        assert result == []

    def test_parse_audit_spec_findings_missing_file(self, tmp_path: Path) -> None:
        result = parse_audit_spec_findings(tmp_path)
        assert result == []


# ---------------------------------------------------------------------------
# Investigation report
# ---------------------------------------------------------------------------


class TestParseInvestigationReport:
    def test_parse_investigation_report(self, tmp_path: Path) -> None:
        data = {
            "trigger": "gate_failed",
            "root_causes": [
                {
                    "description": "Missing import",
                    "confidence": "high",
                    "affected_files": ["src/app.py"],
                    "category": "import_error",
                    "suggested_tasks": [],
                },
            ],
            "unrelated_failures": [],
            "escalation_needed": False,
        }
        (tmp_path / "investigation-report.json").write_text(json.dumps(data))
        result = parse_investigation_report(tmp_path)
        assert isinstance(result, InvestigationReport)
        assert result.trigger == "gate_failed"
        assert len(result.root_causes) == 1

    def test_parse_investigation_report_missing(self, tmp_path: Path) -> None:
        result = parse_investigation_report(tmp_path)
        assert result is None

    def test_parse_investigation_report_invalid(self, tmp_path: Path) -> None:
        (tmp_path / "investigation-report.json").write_text("{bad json!!")
        result = parse_investigation_report(tmp_path)
        assert result is None


# ---------------------------------------------------------------------------
# classify_error
# ---------------------------------------------------------------------------


class TestClassifyError:
    def test_classify_error_signal_error_is_fatal(self) -> None:
        result = classify_error(1, "", "", signal_value="error")
        assert result == ErrorClass.FATAL

    def test_classify_error_exit_137_is_transient(self) -> None:
        result = classify_error(137, "", "", signal_value=None)
        assert result == ErrorClass.TRANSIENT

    def test_classify_error_rate_limit_transient(self) -> None:
        result = classify_error(1, "", "rate limit exceeded", signal_value=None)
        assert result == ErrorClass.TRANSIENT

    def test_classify_error_503_transient(self) -> None:
        result = classify_error(1, "503 Service Unavailable", "", signal_value=None)
        assert result == ErrorClass.TRANSIENT

    def test_classify_error_auth_fatal(self) -> None:
        result = classify_error(1, "", "authentication_error", signal_value=None)
        assert result == ErrorClass.FATAL

    def test_classify_error_permission_denied_fatal(self) -> None:
        result = classify_error(1, "", "permission denied", signal_value=None)
        assert result == ErrorClass.FATAL

    def test_classify_error_unknown_ambiguous(self) -> None:
        result = classify_error(1, "some random output", "", signal_value=None)
        assert result == ErrorClass.AMBIGUOUS


# ---------------------------------------------------------------------------
# map_outcome
# ---------------------------------------------------------------------------


class TestMapOutcome:
    def test_map_outcome_timeout(self) -> None:
        outcome, signal = map_outcome(Phase.DO_TASK, "complete", 0, timed_out=True)
        assert outcome == Outcome.TIMEOUT
        assert signal is None

    def test_map_outcome_gate_pass(self) -> None:
        outcome, signal = map_outcome(Phase.GATE, None, 0, False)
        assert outcome == Outcome.SUCCESS
        assert signal is None

    def test_map_outcome_gate_fail(self) -> None:
        outcome, signal = map_outcome(Phase.GATE, None, 1, False)
        assert outcome == Outcome.FAIL
        assert signal is None

    def test_map_outcome_setup_success(self) -> None:
        outcome, signal = map_outcome(Phase.SETUP, None, 0, False)
        assert outcome == Outcome.SUCCESS
        assert signal is None

    def test_map_outcome_setup_error(self) -> None:
        outcome, signal = map_outcome(Phase.SETUP, None, 1, False)
        assert outcome == Outcome.ERROR
        assert signal is None

    def test_map_outcome_cleanup_success(self) -> None:
        outcome, signal = map_outcome(Phase.CLEANUP, None, 0, False)
        assert outcome == Outcome.SUCCESS
        assert signal is None

    def test_map_outcome_signal_complete(self) -> None:
        outcome, signal = map_outcome(Phase.DO_TASK, "complete", 0, False)
        assert outcome == Outcome.SUCCESS
        assert signal == "complete"

    def test_map_outcome_signal_pass(self) -> None:
        outcome, signal = map_outcome(Phase.AUDIT_TASK, "pass", 0, False)
        assert outcome == Outcome.SUCCESS
        assert signal == "pass"

    def test_map_outcome_signal_all_done(self) -> None:
        outcome, signal = map_outcome(Phase.DO_TASK, "all-done", 0, False)
        assert outcome == Outcome.SUCCESS
        assert signal == "all-done"

    def test_map_outcome_signal_fail(self) -> None:
        outcome, signal = map_outcome(Phase.DO_TASK, "fail", 0, False)
        assert outcome == Outcome.FAIL
        assert signal == "fail"

    def test_map_outcome_signal_blocked(self) -> None:
        outcome, signal = map_outcome(Phase.DO_TASK, "blocked", 0, False)
        assert outcome == Outcome.BLOCKED
        assert signal == "blocked"

    def test_map_outcome_signal_error(self) -> None:
        outcome, signal = map_outcome(Phase.DO_TASK, "error", 1, False)
        assert outcome == Outcome.ERROR
        assert signal == "error"

    def test_map_outcome_unknown_signal(self) -> None:
        outcome, signal = map_outcome(Phase.DO_TASK, "custom-signal", 0, False)
        assert outcome == Outcome.SUCCESS
        assert signal == "custom-signal"

    def test_map_outcome_no_signal(self) -> None:
        outcome, signal = map_outcome(Phase.DO_TASK, None, 1, False)
        assert outcome == Outcome.ERROR
        assert signal is None


# ---------------------------------------------------------------------------
# _extract_audit_spec_signal edge cases
# ---------------------------------------------------------------------------


class TestExtractAuditSpecSignal:
    def test_extract_signal_audit_spec_malformed(self, tmp_path: Path) -> None:
        """Non-matching first line returns None."""
        (tmp_path / "audit.md").write_bytes(b"\x80\x81\x82invalid binary")
        result = extract_signal(Phase.AUDIT_SPEC, "audit-spec", tmp_path, "")
        assert result is None
