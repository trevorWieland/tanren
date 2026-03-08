"""Tests for schemas module."""

import pytest
from pydantic import ValidationError

from worker_manager.schemas import (
    Cli,
    Dispatch,
    Nudge,
    Outcome,
    Phase,
    Result,
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

    def test_phase_count(self):
        assert len(Phase) == 7


class TestCliEnum:
    def test_all_cli_values(self):
        assert Cli.OPENCODE == "opencode"
        assert Cli.CODEX == "codex"
        assert Cli.BASH == "bash"


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
        )
        assert r.outcome == Outcome.SUCCESS
        assert r.signal == "complete"

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
