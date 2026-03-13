"""Tests for schemas module."""

import pytest
from pydantic import ValidationError

from tanren_core.schemas import (
    AuditResult,
    Cli,
    Dispatch,
    Finding,
    FindingSeverity,
    FindingsOutput,
    GateExpectation,
    GateResult,
    InvestigationReport,
    InvestigationRootCause,
    Nudge,
    Outcome,
    Phase,
    ProgressState,
    Result,
    TaskGateExpectation,
    TaskState,
    TaskStatus,
    WorkerHealth,
    WorktreeEntry,
    WorktreeRegistry,
    parse_issue_from_workflow_id,
)


class TestPhaseEnum:
    def test_all_phases(self):
        assert Phase.DO_TASK == "do-task"
        assert Phase.AUDIT_TASK == "audit-task"
        assert Phase.RUN_DEMO == "run-demo"
        assert Phase.AUDIT_SPEC == "audit-spec"
        assert Phase.GATE == "gate"
        assert Phase.SETUP == "setup"
        assert Phase.CLEANUP == "cleanup"

    def test_investigate_phase(self):
        assert Phase.INVESTIGATE == "investigate"

    def test_phase_count(self):
        assert len(Phase) == 8


class TestCliEnum:
    def test_all_cli_values(self):
        assert Cli.OPENCODE == "opencode"
        assert Cli.CODEX == "codex"
        assert Cli.BASH == "bash"

    def test_claude_value(self):
        assert Cli.CLAUDE == "claude"


class TestOutcomeEnum:
    def test_all_outcomes(self):
        assert Outcome.SUCCESS == "success"
        assert Outcome.FAIL == "fail"
        assert Outcome.BLOCKED == "blocked"
        assert Outcome.ERROR == "error"
        assert Outcome.TIMEOUT == "timeout"


class TestDispatch:
    def test_valid_dispatch(self):
        d = Dispatch(
            workflow_id="wf-rentl-144-1741359600",
            phase=Phase.DO_TASK,
            project="rentl",
            spec_folder="tanren/specs/2026-02-19-1531-s0146-slug",
            branch="s0146-slug",
            cli=Cli.OPENCODE,
            model="glm-5",
            gate_cmd=None,
            context=None,
            timeout=1800,
        )
        assert d.workflow_id == "wf-rentl-144-1741359600"
        assert d.phase == Phase.DO_TASK
        assert d.cli == Cli.OPENCODE

    def test_gate_dispatch(self):
        d = Dispatch(
            workflow_id="wf-rentl-144-1741359600",
            phase=Phase.GATE,
            project="rentl",
            spec_folder="tanren/specs/test",
            branch="main",
            cli=Cli.BASH,
            model=None,
            gate_cmd="make check",
            context=None,
            timeout=300,
        )
        assert d.gate_cmd == "make check"
        assert d.model is None

    def test_extra_fields_forbidden(self):
        with pytest.raises(ValidationError):
            Dispatch(
                workflow_id="wf-rentl-144-1741359600",
                phase=Phase.DO_TASK,
                project="rentl",
                spec_folder="tanren/specs/test",
                branch="main",
                cli=Cli.OPENCODE,
                model="glm-5",
                gate_cmd=None,
                context=None,
                timeout=1800,
                extra_field="not allowed",
            )

    def test_invalid_phase(self):
        with pytest.raises(ValidationError):
            Dispatch(
                workflow_id="wf-rentl-144-1741359600",
                phase="invalid",
                project="rentl",
                spec_folder="tanren/specs/test",
                branch="main",
                cli=Cli.OPENCODE,
                model="glm-5",
                gate_cmd=None,
                context=None,
                timeout=1800,
            )

    def test_roundtrip_json(self):
        d = Dispatch(
            workflow_id="wf-rentl-144-1741359600",
            phase=Phase.DO_TASK,
            project="rentl",
            spec_folder="tanren/specs/test",
            branch="s0146-slug",
            cli=Cli.OPENCODE,
            model="glm-5",
            gate_cmd=None,
            context=None,
            timeout=1800,
        )
        json_str = d.model_dump_json()
        d2 = Dispatch.model_validate_json(json_str)
        assert d == d2


class TestResult:
    def test_valid_result(self):
        r = Result(
            workflow_id="wf-rentl-144-1741359600",
            phase=Phase.DO_TASK,
            outcome=Outcome.SUCCESS,
            signal="complete",
            exit_code=0,
            duration_secs=342,
            gate_output=None,
            tail_output=None,
            unchecked_tasks=2,
            plan_hash="a3f2b8c1",
            spec_modified=False,
            pushed=True,
        )
        assert r.outcome == Outcome.SUCCESS
        assert r.signal == "complete"
        assert r.pushed is True

    def test_pushed_defaults_to_none(self):
        r = Result(
            workflow_id="wf-rentl-144-1741359600",
            phase=Phase.SETUP,
            outcome=Outcome.SUCCESS,
            signal=None,
            exit_code=0,
            duration_secs=1,
            gate_output=None,
            tail_output=None,
            unchecked_tasks=0,
            plan_hash="00000000",
            spec_modified=False,
        )
        assert r.pushed is None

    def test_gate_result_with_output(self):
        r = Result(
            workflow_id="wf-rentl-144-1741359600",
            phase=Phase.GATE,
            outcome=Outcome.FAIL,
            signal=None,
            exit_code=1,
            duration_secs=87,
            gate_output="FAILED tests/unit/test_foo.py::test_bar",
            tail_output=None,
            unchecked_tasks=2,
            plan_hash="a3f2b8c1",
            spec_modified=False,
        )
        assert r.gate_output is not None
        assert r.pushed is None


class TestWorkerHealth:
    def test_valid_health(self):
        h = WorkerHealth(
            pid=1234,
            started_at="2026-03-08T10:00:00Z",
            last_poll="2026-03-08T10:01:00Z",
            active_processes=1,
            queued_dispatches=0,
        )
        assert h.alive is True
        assert h.pid == 1234
        assert h.active_processes == 1

    def test_health_json_roundtrip(self):
        h = WorkerHealth(
            pid=5678,
            started_at="2026-03-08T10:00:00Z",
            last_poll="2026-03-08T10:02:00Z",
            active_processes=2,
            queued_dispatches=3,
        )
        json_str = h.model_dump_json()
        h2 = WorkerHealth.model_validate_json(json_str)
        assert h == h2


class TestNudge:
    def test_nudge_defaults(self):
        n = Nudge(workflow_id="wf-rentl-144-1741359600")
        assert n.type == "workflow_result"
        assert n.workflow_id == "wf-rentl-144-1741359600"

    def test_nudge_json(self):
        n = Nudge(workflow_id="wf-rentl-144-1741359600")
        data = n.model_dump()
        assert data == {"type": "workflow_result", "workflow_id": "wf-rentl-144-1741359600"}


class TestWorktreeRegistry:
    def test_empty_registry(self):
        r = WorktreeRegistry()
        assert r.worktrees == {}

    def test_registry_with_entry(self):
        entry = WorktreeEntry(
            project="rentl",
            issue=144,
            branch="s0146-slug",
            path="/home/trevor/github/rentl-wt-144",
            created_at="2026-03-07T15:01:00Z",
        )
        r = WorktreeRegistry(worktrees={"wf-rentl-144-1741359600": entry})
        assert "wf-rentl-144-1741359600" in r.worktrees
        assert r.worktrees["wf-rentl-144-1741359600"].issue == 144


class TestParseIssueFromWorkflowId:
    def test_simple(self):
        assert parse_issue_from_workflow_id("wf-rentl-144-1741359600") == 144

    def test_hyphenated_project(self):
        assert parse_issue_from_workflow_id("wf-unicorn-armada-3-1741359600") == 3

    def test_invalid_format(self):
        with pytest.raises(ValueError, match="Invalid workflow_id format"):
            parse_issue_from_workflow_id("invalid")

    def test_missing_prefix(self):
        with pytest.raises(ValueError):
            parse_issue_from_workflow_id("rentl-144-1741359600")


class TestTaskStatus:
    def test_all_values(self):
        assert TaskStatus.PENDING == "pending"
        assert TaskStatus.IN_PROGRESS == "in_progress"
        assert TaskStatus.GATE_PASSED == "gate_passed"
        assert TaskStatus.COMPLETED == "completed"
        assert TaskStatus.BLOCKED == "blocked"
        assert TaskStatus.FAILED == "failed"

    def test_count(self):
        assert len(TaskStatus) == 6


class TestGateResult:
    def test_valid(self):
        g = GateResult(attempt=1, passed=True)
        assert g.attempt == 1
        assert g.passed is True
        assert g.must_pass_failures == []
        assert g.unexpected_passes == []

    def test_extra_forbid(self):
        with pytest.raises(ValidationError):
            GateResult(attempt=1, passed=True, extra="bad")


class TestAuditResult:
    def test_valid(self):
        a = AuditResult(attempt=1, signal="pass")
        assert a.signal == "pass"
        assert a.findings == []

    def test_defaults(self):
        a = AuditResult(attempt=2, signal=None)
        assert a.findings == []


class TestTaskState:
    def test_defaults(self):
        t = TaskState(id=1, title="Setup project")
        assert t.status == TaskStatus.PENDING
        assert t.attempts == 0
        assert t.gate_results == []
        assert t.audit_results == []
        assert t.gate_expectations is None
        assert t.source is None

    def test_status_transitions(self):
        t = TaskState(id=1, title="Test")
        t.status = TaskStatus.IN_PROGRESS
        assert t.status == TaskStatus.IN_PROGRESS
        t.status = TaskStatus.COMPLETED
        assert t.status == TaskStatus.COMPLETED


class TestProgressState:
    def test_valid(self):
        p = ProgressState(
            spec_id="s0146",
            created_at="2026-03-08T10:00:00Z",
            updated_at="2026-03-08T10:00:00Z",
            tasks=[
                TaskState(id=1, title="Setup"),
                TaskState(id=2, title="Implement"),
            ],
        )
        assert p.spec_id == "s0146"
        assert len(p.tasks) == 2
        assert p.version == 1

    def test_empty_tasks(self):
        p = ProgressState(
            spec_id="s0001",
            created_at="2026-03-08T10:00:00Z",
            updated_at="2026-03-08T10:00:00Z",
            tasks=[],
        )
        assert p.tasks == []

    def test_json_roundtrip(self):
        p = ProgressState(
            spec_id="s0146",
            created_at="2026-03-08T10:00:00Z",
            updated_at="2026-03-08T10:00:00Z",
            tasks=[TaskState(id=1, title="Setup")],
        )
        json_str = p.model_dump_json()
        p2 = ProgressState.model_validate_json(json_str)
        assert p == p2


class TestResultExtended:
    def test_integrity_repairs_default(self):
        """integrity_repairs defaults to None for backward compat."""
        r = Result(
            workflow_id="wf-rentl-144-1741359600",
            phase=Phase.DO_TASK,
            outcome=Outcome.SUCCESS,
            signal="complete",
            exit_code=0,
            duration_secs=100,
            gate_output=None,
            tail_output=None,
            unchecked_tasks=0,
            plan_hash="a3f2b8c1",
            spec_modified=False,
        )
        assert r.integrity_repairs is None
        assert r.new_tasks == []

    def test_integrity_repairs_set(self):
        r = Result(
            workflow_id="wf-rentl-144-1741359600",
            phase=Phase.DO_TASK,
            outcome=Outcome.SUCCESS,
            signal="complete",
            exit_code=0,
            duration_secs=100,
            gate_output=None,
            tail_output=None,
            unchecked_tasks=0,
            plan_hash="a3f2b8c1",
            spec_modified=False,
            integrity_repairs={"spec_reverted": True},
            new_tasks=[{"id": 10, "title": "Fix bug"}],
        )
        assert r.integrity_repairs.spec_reverted is True
        assert len(r.new_tasks) == 1

    def test_backward_compat_no_new_fields(self):
        """Constructing without the new fields should work."""
        r = Result(
            workflow_id="wf-rentl-144-1741359600",
            phase=Phase.SETUP,
            outcome=Outcome.SUCCESS,
            signal=None,
            exit_code=0,
            duration_secs=1,
            gate_output=None,
            tail_output=None,
            unchecked_tasks=0,
            plan_hash="00000000",
            spec_modified=False,
        )
        assert r.integrity_repairs is None
        assert r.new_tasks == []
        assert r.findings == []


class TestFindingSeverity:
    def test_values(self):
        assert FindingSeverity.FIX == "fix"
        assert FindingSeverity.NOTE == "note"
        assert FindingSeverity.QUESTION == "question"

    def test_count(self):
        assert len(FindingSeverity) == 3


class TestFinding:
    def test_valid(self):
        f = Finding(title="Missing error handling")
        assert f.title == "Missing error handling"
        assert f.severity == FindingSeverity.FIX
        assert f.affected_files == []
        assert f.line_numbers == []

    def test_full(self):
        f = Finding(
            title="Bug",
            description="Details",
            severity=FindingSeverity.NOTE,
            affected_files=["src/foo.py"],
            line_numbers=[42],
        )
        assert f.severity == FindingSeverity.NOTE
        assert f.affected_files == ["src/foo.py"]

    def test_extra_forbid(self):
        with pytest.raises(ValidationError):
            Finding(title="Bug", extra="bad")


class TestFindingsOutput:
    def test_valid(self):
        fo = FindingsOutput(signal="pass")
        assert fo.signal == "pass"
        assert fo.findings == []

    def test_with_findings(self):
        fo = FindingsOutput(
            signal="fail",
            findings=[Finding(title="Issue 1"), Finding(title="Issue 2")],
        )
        assert len(fo.findings) == 2
        assert fo.findings[0].title == "Issue 1"

    def test_json_roundtrip(self):
        fo = FindingsOutput(
            signal="fail",
            findings=[Finding(title="Bug", severity=FindingSeverity.FIX)],
        )
        json_str = fo.model_dump_json()
        fo2 = FindingsOutput.model_validate_json(json_str)
        assert fo == fo2


class TestInvestigationReport:
    def test_valid(self):
        report = InvestigationReport(trigger="gate_failure_persistent")
        assert report.trigger == "gate_failure_persistent"
        assert report.root_causes == []
        assert report.escalation_needed is False

    def test_with_root_causes(self):
        rc = InvestigationRootCause(
            description="Wrong function call",
            confidence="high",
            affected_files=["src/foo.py"],
            category="code_bug",
            suggested_tasks=[{"title": "Fix call"}],
        )
        report = InvestigationReport(
            trigger="gate_failure_persistent",
            root_causes=[rc],
        )
        assert len(report.root_causes) == 1
        assert report.root_causes[0].confidence == "high"

    def test_escalation(self):
        report = InvestigationReport(
            trigger="demo_failure",
            escalation_needed=True,
            escalation_reason="Spec is ambiguous",
        )
        assert report.escalation_needed is True


class TestGateExpectation:
    def test_defaults(self):
        ge = GateExpectation()
        assert ge.must_pass == []
        assert ge.expect_fail == []
        assert ge.skip == []
        assert ge.gate_command_override is None

    def test_full(self):
        ge = GateExpectation(
            must_pass=["lint", "typecheck", "unit:foo"],
            expect_fail=["integration:bar"],
            skip=["unit:baz"],
            gate_command_override="make all",
        )
        assert "lint" in ge.must_pass
        assert ge.gate_command_override == "make all"

    def test_wildcard(self):
        ge = GateExpectation(must_pass=["*"])
        assert "*" in ge.must_pass


class TestTaskGateExpectation:
    def test_valid(self):
        tge = TaskGateExpectation(
            task_id=2,
            title="Write module A",
            gate=GateExpectation(must_pass=["lint"]),
        )
        assert tge.task_id == 2
        assert tge.gate.must_pass == ["lint"]
