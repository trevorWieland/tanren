"""Integration tests for WorkerManager dispatch handling, findings parsing, and result writing."""

import json
from pathlib import Path
from unittest.mock import AsyncMock

import pytest

from tanren_core.adapters.null_emitter import NullEventEmitter
from tanren_core.adapters.types import (
    CustomEnvironmentRuntime,
    EnvironmentHandle,
    LocalEnvironmentRuntime,
    PhaseResult,
    ProvisionError,
)
from tanren_core.config import Config
from tanren_core.manager import (
    WorkerManager,
    _build_gate_output,  # noqa: PLC2701
    build_tail_output,
)
from tanren_core.postflight import IntegrityRepairs, PostflightResult
from tanren_core.preflight import PreflightResult
from tanren_core.schemas import (
    Cli,
    Dispatch,
    Outcome,
    Phase,
    Result,
)


def _make_config(tmp_path: Path) -> Config:
    """Create a minimal Config pointing at tmp_path subdirs."""
    return Config(
        ipc_dir=str(tmp_path / "ipc"),
        github_dir=str(tmp_path / "github"),
        data_dir=str(tmp_path / "data"),
        worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
    )


def _make_dispatch(
    *,
    phase: Phase = Phase.DO_TASK,
    workflow_id: str = "wf-demo-42-1700000000",
    project: str = "demo",
    spec_folder: str = "specs/my-spec",
    branch: str = "tanren/demo-42",
    cli: Cli = Cli.CLAUDE,
    timeout: int = 300,
) -> Dispatch:
    """Create a Dispatch with sensible defaults."""
    return Dispatch(
        workflow_id=workflow_id,
        phase=phase,
        project=project,
        spec_folder=spec_folder,
        branch=branch,
        cli=cli,
        timeout=timeout,
    )


def _make_manager(
    tmp_path: Path,
    *,
    execution_env: AsyncMock | None = None,
    worktree_mgr: AsyncMock | None = None,
    emitter: NullEventEmitter | None = None,
) -> WorkerManager:
    """Create a WorkerManager with injected mocks."""
    config = _make_config(tmp_path)
    # Ensure IPC dirs exist so result writing works
    for d in ("dispatch", "results", "in-progress", "input"):
        (tmp_path / "ipc" / d).mkdir(parents=True, exist_ok=True)
    (tmp_path / "github").mkdir(parents=True, exist_ok=True)
    return WorkerManager(
        config,
        execution_env=execution_env or AsyncMock(),
        worktree_mgr=worktree_mgr or AsyncMock(),
        emitter=emitter or NullEventEmitter(),
    )


# ---------------------------------------------------------------------------
# _handle_dispatch routing
# ---------------------------------------------------------------------------


class TestHandleDispatchRouting:
    """Verify _handle_dispatch routes to setup, cleanup, or work-phase handlers."""

    @pytest.mark.asyncio
    async def test_setup_phase_creates_worktree_and_writes_result(self, tmp_path: Path):
        wt_mgr = AsyncMock()
        wt_mgr.create.return_value = tmp_path / "github" / "demo-wt-42"
        manager = _make_manager(tmp_path, worktree_mgr=wt_mgr)

        dispatch = _make_dispatch(phase=Phase.SETUP)
        dispatch_path = tmp_path / "ipc" / "dispatch" / "1700000000-aaaaaa.json"
        dispatch_path.write_text(dispatch.model_dump_json())

        await manager._handle_dispatch(dispatch_path, dispatch)

        wt_mgr.create.assert_awaited_once()
        # Result file should appear in results/
        result_files = list((tmp_path / "ipc" / "results").iterdir())
        assert len(result_files) == 1
        result_data = json.loads(result_files[0].read_text())
        assert result_data["phase"] == "setup"
        assert result_data["outcome"] == "success"

    @pytest.mark.asyncio
    async def test_cleanup_phase_calls_cleanup_and_writes_result(self, tmp_path: Path):
        wt_mgr = AsyncMock()
        manager = _make_manager(tmp_path, worktree_mgr=wt_mgr)

        dispatch = _make_dispatch(phase=Phase.CLEANUP)
        dispatch_path = tmp_path / "ipc" / "dispatch" / "1700000000-bbbbbb.json"
        dispatch_path.write_text(dispatch.model_dump_json())

        await manager._handle_dispatch(dispatch_path, dispatch)

        wt_mgr.cleanup.assert_awaited_once()
        result_files = list((tmp_path / "ipc" / "results").iterdir())
        assert len(result_files) == 1
        result_data = json.loads(result_files[0].read_text())
        assert result_data["phase"] == "cleanup"
        assert result_data["outcome"] == "success"

    @pytest.mark.asyncio
    async def test_work_phase_provisions_executes_and_writes_result(self, tmp_path: Path):
        """Work phases call execution_env.provision → execute → teardown."""
        exec_env = AsyncMock()
        worktree_path = tmp_path / "github" / "demo-wt-42"
        worktree_path.mkdir(parents=True)
        spec_path = worktree_path / "specs" / "my-spec"
        spec_path.mkdir(parents=True)

        handle = EnvironmentHandle(
            env_id="test-env",
            worktree_path=worktree_path,
            branch="tanren/demo-42",
            project="demo",
            runtime=LocalEnvironmentRuntime(),
        )
        exec_env.provision.return_value = handle
        exec_env.execute.return_value = PhaseResult(
            outcome=Outcome.SUCCESS,
            signal="complete",
            exit_code=0,
            stdout="all good",
            duration_secs=5,
            preflight_passed=True,
        )

        manager = _make_manager(tmp_path, execution_env=exec_env)

        dispatch = _make_dispatch(phase=Phase.DO_TASK)
        dispatch_path = tmp_path / "ipc" / "dispatch" / "1700000000-cccccc.json"
        dispatch_path.write_text(dispatch.model_dump_json())

        await manager._handle_dispatch(dispatch_path, dispatch)

        exec_env.provision.assert_awaited_once()
        exec_env.execute.assert_awaited_once()
        exec_env.teardown.assert_awaited_once()

        result_files = list((tmp_path / "ipc" / "results").iterdir())
        assert len(result_files) == 1
        result_data = json.loads(result_files[0].read_text())
        assert result_data["outcome"] == "success"
        assert result_data["signal"] == "complete"

    @pytest.mark.asyncio
    async def test_setup_failure_writes_error_result(self, tmp_path: Path):
        wt_mgr = AsyncMock()
        wt_mgr.create.side_effect = RuntimeError("git worktree add failed")
        manager = _make_manager(tmp_path, worktree_mgr=wt_mgr)

        dispatch = _make_dispatch(phase=Phase.SETUP)
        dispatch_path = tmp_path / "ipc" / "dispatch" / "1700000000-dddddd.json"
        dispatch_path.write_text(dispatch.model_dump_json())

        await manager._handle_dispatch(dispatch_path, dispatch)

        result_files = list((tmp_path / "ipc" / "results").iterdir())
        assert len(result_files) == 1
        result_data = json.loads(result_files[0].read_text())
        assert result_data["phase"] == "setup"
        assert result_data["outcome"] == "error"
        assert "git worktree add failed" in result_data["tail_output"]

    @pytest.mark.asyncio
    async def test_cleanup_failure_writes_error_result(self, tmp_path: Path):
        wt_mgr = AsyncMock()
        wt_mgr.cleanup.side_effect = RuntimeError("cleanup exploded")
        manager = _make_manager(tmp_path, worktree_mgr=wt_mgr)

        dispatch = _make_dispatch(phase=Phase.CLEANUP)
        dispatch_path = tmp_path / "ipc" / "dispatch" / "1700000000-eeeeee.json"
        dispatch_path.write_text(dispatch.model_dump_json())

        await manager._handle_dispatch(dispatch_path, dispatch)

        result_files = list((tmp_path / "ipc" / "results").iterdir())
        assert len(result_files) == 1
        result_data = json.loads(result_files[0].read_text())
        assert result_data["phase"] == "cleanup"
        assert result_data["outcome"] == "error"
        assert "cleanup exploded" in result_data["tail_output"]

    @pytest.mark.asyncio
    async def test_unhandled_exception_writes_error_result(self, tmp_path: Path):
        """An unexpected exception in _handle_dispatch writes an error Result."""
        exec_env = AsyncMock()
        exec_env.provision.side_effect = RuntimeError("kaboom")

        manager = _make_manager(tmp_path, execution_env=exec_env)

        dispatch = _make_dispatch(phase=Phase.DO_TASK)
        dispatch_path = tmp_path / "ipc" / "dispatch" / "1700000000-ffffff.json"
        dispatch_path.write_text(dispatch.model_dump_json())

        await manager._handle_dispatch(dispatch_path, dispatch)

        result_files = list((tmp_path / "ipc" / "results").iterdir())
        assert len(result_files) == 1
        result_data = json.loads(result_files[0].read_text())
        assert result_data["outcome"] == "error"
        assert result_data["exit_code"] == -1
        assert result_data["tail_output"] == "Worker manager internal error"

    @pytest.mark.asyncio
    async def test_provision_error_writes_preflight_failed_result(self, tmp_path: Path):
        """ProvisionError (e.g. failed preflight) writes the error's result directly."""
        error_result = Result(
            workflow_id="wf-demo-42-1700000000",
            phase=Phase.DO_TASK,
            outcome=Outcome.ERROR,
            signal=None,
            exit_code=1,
            duration_secs=0,
            gate_output=None,
            tail_output="Preflight failed: missing command file",
            unchecked_tasks=0,
            plan_hash="00000000",
            spec_modified=False,
        )
        exec_env = AsyncMock()
        exec_env.provision.side_effect = ProvisionError(error_result)

        manager = _make_manager(tmp_path, execution_env=exec_env)

        dispatch = _make_dispatch(phase=Phase.DO_TASK)
        dispatch_path = tmp_path / "ipc" / "dispatch" / "1700000000-111111.json"
        dispatch_path.write_text(dispatch.model_dump_json())

        await manager._handle_dispatch(dispatch_path, dispatch)

        exec_env.execute.assert_not_awaited()
        result_files = list((tmp_path / "ipc" / "results").iterdir())
        assert len(result_files) == 1
        result_data = json.loads(result_files[0].read_text())
        assert result_data["outcome"] == "error"
        assert "Preflight failed" in result_data["tail_output"]


# ---------------------------------------------------------------------------
# _handle_work_phase: gate output, tail output, postflight
# ---------------------------------------------------------------------------


class TestWorkPhaseDetails:
    """Test work-phase behavior: gate output, tail output, postflight wiring."""

    @pytest.mark.asyncio
    async def test_gate_phase_produces_gate_output(self, tmp_path: Path):
        exec_env = AsyncMock()
        worktree_path = tmp_path / "github" / "demo-wt-42"
        worktree_path.mkdir(parents=True)
        spec_path = worktree_path / "specs" / "my-spec"
        spec_path.mkdir(parents=True)

        handle = EnvironmentHandle(
            env_id="test-env",
            worktree_path=worktree_path,
            branch="tanren/demo-42",
            project="demo",
            runtime=LocalEnvironmentRuntime(),
        )
        exec_env.provision.return_value = handle
        stdout_lines = "\n".join(f"line {i}" for i in range(200))
        exec_env.execute.return_value = PhaseResult(
            outcome=Outcome.SUCCESS,
            signal=None,
            exit_code=0,
            stdout=stdout_lines,
            duration_secs=3,
            preflight_passed=True,
        )

        manager = _make_manager(tmp_path, execution_env=exec_env)

        dispatch = _make_dispatch(phase=Phase.GATE)
        dispatch_path = tmp_path / "ipc" / "dispatch" / "1700000000-222222.json"
        dispatch_path.write_text(dispatch.model_dump_json())

        await manager._handle_dispatch(dispatch_path, dispatch)

        result_files = list((tmp_path / "ipc" / "results").iterdir())
        result_data = json.loads(result_files[0].read_text())
        assert result_data["gate_output"] is not None
        # Success: last 100 lines
        gate_lines = result_data["gate_output"].split("\n")
        assert len(gate_lines) == 100

    @pytest.mark.asyncio
    async def test_agent_phase_produces_tail_output(self, tmp_path: Path):
        exec_env = AsyncMock()
        worktree_path = tmp_path / "github" / "demo-wt-42"
        worktree_path.mkdir(parents=True)
        spec_path = worktree_path / "specs" / "my-spec"
        spec_path.mkdir(parents=True)

        handle = EnvironmentHandle(
            env_id="test-env",
            worktree_path=worktree_path,
            branch="tanren/demo-42",
            project="demo",
            runtime=LocalEnvironmentRuntime(),
        )
        exec_env.provision.return_value = handle
        exec_env.execute.return_value = PhaseResult(
            outcome=Outcome.SUCCESS,
            signal="complete",
            exit_code=0,
            stdout="agent output line 1\nagent output line 2",
            duration_secs=10,
            preflight_passed=True,
        )

        manager = _make_manager(tmp_path, execution_env=exec_env)

        dispatch = _make_dispatch(phase=Phase.DO_TASK)
        dispatch_path = tmp_path / "ipc" / "dispatch" / "1700000000-333333.json"
        dispatch_path.write_text(dispatch.model_dump_json())

        await manager._handle_dispatch(dispatch_path, dispatch)

        result_files = list((tmp_path / "ipc" / "results").iterdir())
        result_data = json.loads(result_files[0].read_text())
        assert result_data["tail_output"] == "agent output line 1\nagent output line 2"

    @pytest.mark.asyncio
    async def test_postflight_push_failure_appends_to_tail_output(self, tmp_path: Path):
        exec_env = AsyncMock()
        worktree_path = tmp_path / "github" / "demo-wt-42"
        worktree_path.mkdir(parents=True)
        spec_path = worktree_path / "specs" / "my-spec"
        spec_path.mkdir(parents=True)

        handle = EnvironmentHandle(
            env_id="test-env",
            worktree_path=worktree_path,
            branch="tanren/demo-42",
            project="demo",
            runtime=LocalEnvironmentRuntime(),
        )
        exec_env.provision.return_value = handle
        exec_env.execute.return_value = PhaseResult(
            outcome=Outcome.SUCCESS,
            signal="complete",
            exit_code=0,
            stdout="some output",
            duration_secs=10,
            preflight_passed=True,
            postflight_result=PostflightResult(
                pushed=False,
                push_error="remote rejected",
                integrity_repairs=IntegrityRepairs(spec_reverted=True),
            ),
        )

        manager = _make_manager(tmp_path, execution_env=exec_env)

        dispatch = _make_dispatch(phase=Phase.DO_TASK)
        dispatch_path = tmp_path / "ipc" / "dispatch" / "1700000000-444444.json"
        dispatch_path.write_text(dispatch.model_dump_json())

        await manager._handle_dispatch(dispatch_path, dispatch)

        result_files = list((tmp_path / "ipc" / "results").iterdir())
        result_data = json.loads(result_files[0].read_text())
        assert "remote rejected" in result_data["tail_output"]
        assert result_data["spec_modified"] is True
        assert result_data["pushed"] is False

    @pytest.mark.asyncio
    async def test_postflight_push_failure_no_prior_tail_output(self, tmp_path: Path):
        """When tail_output is None and push fails, tail_output shows push error."""
        exec_env = AsyncMock()
        worktree_path = tmp_path / "github" / "demo-wt-42"
        worktree_path.mkdir(parents=True)
        spec_path = worktree_path / "specs" / "my-spec"
        spec_path.mkdir(parents=True)

        handle = EnvironmentHandle(
            env_id="test-env",
            worktree_path=worktree_path,
            branch="tanren/demo-42",
            project="demo",
            runtime=LocalEnvironmentRuntime(),
        )
        exec_env.provision.return_value = handle
        exec_env.execute.return_value = PhaseResult(
            outcome=Outcome.SUCCESS,
            signal="complete",
            exit_code=0,
            stdout=None,
            duration_secs=10,
            preflight_passed=True,
            postflight_result=PostflightResult(
                pushed=False,
                push_error="auth denied",
                integrity_repairs=IntegrityRepairs(),
            ),
        )

        manager = _make_manager(tmp_path, execution_env=exec_env)

        dispatch = _make_dispatch(phase=Phase.GATE)
        dispatch_path = tmp_path / "ipc" / "dispatch" / "1700000000-555555.json"
        dispatch_path.write_text(dispatch.model_dump_json())

        await manager._handle_dispatch(dispatch_path, dispatch)

        result_files = list((tmp_path / "ipc" / "results").iterdir())
        result_data = json.loads(result_files[0].read_text())
        assert result_data["tail_output"] == "git push failed: auth denied"

    @pytest.mark.asyncio
    async def test_execute_exception_writes_error_result_and_calls_teardown(self, tmp_path: Path):
        """If execute() raises, an error result is written and teardown is still called."""
        exec_env = AsyncMock()
        worktree_path = tmp_path / "github" / "demo-wt-42"
        worktree_path.mkdir(parents=True)

        handle = EnvironmentHandle(
            env_id="test-env",
            worktree_path=worktree_path,
            branch="tanren/demo-42",
            project="demo",
            runtime=LocalEnvironmentRuntime(),
        )
        exec_env.provision.return_value = handle
        exec_env.execute.side_effect = RuntimeError("execute crashed")

        manager = _make_manager(tmp_path, execution_env=exec_env)

        dispatch = _make_dispatch(phase=Phase.DO_TASK)
        dispatch_path = tmp_path / "ipc" / "dispatch" / "1700000000-666666.json"
        dispatch_path.write_text(dispatch.model_dump_json())

        await manager._handle_dispatch(dispatch_path, dispatch)

        exec_env.teardown.assert_awaited_once()
        result_files = list((tmp_path / "ipc" / "results").iterdir())
        result_data = json.loads(result_files[0].read_text())
        assert result_data["outcome"] == "error"
        assert "execute crashed" in result_data["tail_output"]


# ---------------------------------------------------------------------------
# _parse_findings — real files in tmp_path
# ---------------------------------------------------------------------------


class TestParseFindings:
    """Integration tests for _parse_findings using real files on disk."""

    def test_audit_task_findings_parsed(self, tmp_path: Path):
        spec_folder = tmp_path / "specs" / "spec-1"
        spec_folder.mkdir(parents=True)

        findings = {
            "signal": "fail",
            "findings": [
                {
                    "title": "Missing error handling",
                    "description": "No try/except around DB call",
                    "severity": "fix",
                    "affected_files": ["app.py"],
                    "line_numbers": [42],
                },
                {
                    "title": "Style note",
                    "description": "Consider using f-strings",
                    "severity": "note",
                    "affected_files": ["utils.py"],
                    "line_numbers": [],
                },
            ],
        }
        (spec_folder / "audit-findings.json").write_text(json.dumps(findings))

        manager = _make_manager(tmp_path)
        dispatch = _make_dispatch(phase=Phase.AUDIT_TASK)
        new_tasks, findings_data = manager._parse_findings(dispatch, spec_folder)

        # Both findings should appear in findings_data
        assert len(findings_data) == 2
        # Only "fix" severity should appear in new_tasks
        assert len(new_tasks) == 1
        assert new_tasks[0]["title"] == "Missing error handling"

    def test_run_demo_findings_parsed(self, tmp_path: Path):
        spec_folder = tmp_path / "specs" / "spec-1"
        spec_folder.mkdir(parents=True)

        findings = {
            "signal": "fail",
            "findings": [
                {
                    "title": "Button not clickable",
                    "severity": "fix",
                },
            ],
        }
        (spec_folder / "demo-findings.json").write_text(json.dumps(findings))

        manager = _make_manager(tmp_path)
        dispatch = _make_dispatch(phase=Phase.RUN_DEMO)
        new_tasks, findings_data = manager._parse_findings(dispatch, spec_folder)

        assert len(findings_data) == 1
        assert len(new_tasks) == 1

    def test_audit_spec_findings_parsed_from_audit_md(self, tmp_path: Path):
        spec_folder = tmp_path / "specs" / "spec-1"
        spec_folder.mkdir(parents=True)

        audit_content = (
            "status: fail\n"
            "\n"
            "Some audit text here.\n"
            "\n"
            "<!-- structured-findings-start -->\n"
            '[{"title": "Vague requirement", "severity": "fix"},'
            ' {"title": "Minor ambiguity", "severity": "note"}]\n'
            "<!-- structured-findings-end -->\n"
        )
        (spec_folder / "audit.md").write_text(audit_content)

        manager = _make_manager(tmp_path)
        dispatch = _make_dispatch(phase=Phase.AUDIT_SPEC)
        new_tasks, findings_data = manager._parse_findings(dispatch, spec_folder)

        assert len(findings_data) == 2
        assert len(new_tasks) == 1
        assert new_tasks[0]["title"] == "Vague requirement"

    def test_investigate_findings_parsed(self, tmp_path: Path):
        spec_folder = tmp_path / "specs" / "spec-1"
        spec_folder.mkdir(parents=True)

        report = {
            "trigger": "test failure",
            "root_causes": [
                {
                    "description": "Race condition in DB pool",
                    "confidence": "high",
                    "affected_files": ["pool.py"],
                    "category": "concurrency",
                    "suggested_tasks": [
                        {"title": "Add mutex to pool init"},
                    ],
                },
            ],
            "unrelated_failures": [],
            "escalation_needed": False,
        }
        (spec_folder / "investigation-report.json").write_text(json.dumps(report))

        manager = _make_manager(tmp_path)
        dispatch = _make_dispatch(phase=Phase.INVESTIGATE)
        new_tasks, findings_data = manager._parse_findings(dispatch, spec_folder)

        assert len(findings_data) == 1
        assert "report" in findings_data[0]
        assert len(new_tasks) == 1
        assert new_tasks[0]["title"] == "Add mutex to pool init"

    def test_no_findings_file_returns_empty(self, tmp_path: Path):
        spec_folder = tmp_path / "specs" / "spec-1"
        spec_folder.mkdir(parents=True)

        manager = _make_manager(tmp_path)
        dispatch = _make_dispatch(phase=Phase.AUDIT_TASK)
        new_tasks, findings_data = manager._parse_findings(dispatch, spec_folder)

        assert new_tasks == []
        assert findings_data == []

    def test_do_task_phase_skips_findings(self, tmp_path: Path):
        """DO_TASK phase does not parse any findings files."""
        spec_folder = tmp_path / "specs" / "spec-1"
        spec_folder.mkdir(parents=True)
        # Even if a findings file exists, DO_TASK should not parse it
        (spec_folder / "audit-findings.json").write_text(
            '{"signal": "fail", "findings": [{"title": "X", "severity": "fix"}]}'
        )

        manager = _make_manager(tmp_path)
        dispatch = _make_dispatch(phase=Phase.DO_TASK)
        new_tasks, findings_data = manager._parse_findings(dispatch, spec_folder)

        assert new_tasks == []
        assert findings_data == []

    def test_gate_phase_skips_findings(self, tmp_path: Path):
        """GATE phase does not parse any findings files."""
        spec_folder = tmp_path / "specs" / "spec-1"
        spec_folder.mkdir(parents=True)

        manager = _make_manager(tmp_path)
        dispatch = _make_dispatch(phase=Phase.GATE)
        new_tasks, findings_data = manager._parse_findings(dispatch, spec_folder)

        assert new_tasks == []
        assert findings_data == []


# ---------------------------------------------------------------------------
# Result writing and nudge emission
# ---------------------------------------------------------------------------


class TestResultWriting:
    """Verify result files and nudge files are written correctly."""

    @pytest.mark.asyncio
    async def test_result_file_is_valid_json_with_all_fields(self, tmp_path: Path):
        manager = _make_manager(tmp_path)

        result = Result(
            workflow_id="wf-demo-42-1700000000",
            phase=Phase.DO_TASK,
            outcome=Outcome.SUCCESS,
            signal="complete",
            exit_code=0,
            duration_secs=120,
            gate_output=None,
            tail_output="final line",
            unchecked_tasks=2,
            plan_hash="abcdef01",
            spec_modified=False,
            pushed=True,
        )
        await manager._write_result_and_nudge(result, "wf-demo-42-1700000000")

        result_files = list((tmp_path / "ipc" / "results").iterdir())
        assert len(result_files) == 1
        data = json.loads(result_files[0].read_text())
        assert data["workflow_id"] == "wf-demo-42-1700000000"
        assert data["phase"] == "do-task"
        assert data["outcome"] == "success"
        assert data["signal"] == "complete"
        assert data["unchecked_tasks"] == 2
        assert data["plan_hash"] == "abcdef01"
        assert data["pushed"] is True

    @pytest.mark.asyncio
    async def test_nudge_file_written_to_input_dir(self, tmp_path: Path):
        manager = _make_manager(tmp_path)

        result = Result(
            workflow_id="wf-demo-42-1700000000",
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
        await manager._write_result_and_nudge(result, "wf-demo-42-1700000000")

        nudge_files = list((tmp_path / "ipc" / "input").iterdir())
        assert len(nudge_files) == 1
        envelope = json.loads(nudge_files[0].read_text())
        assert envelope["type"] == "message"
        nudge_data = json.loads(envelope["text"])
        assert nudge_data["workflow_id"] == "wf-demo-42-1700000000"


# ---------------------------------------------------------------------------
# _preflight_repairs helper
# ---------------------------------------------------------------------------


class TestPreflightRepairs:
    def test_returns_repairs_for_local_runtime(self, tmp_path: Path):
        manager = _make_manager(tmp_path)

        preflight_result = PreflightResult(
            passed=True,
            repairs=["Switched branch to tanren/demo-42", "Cleared .agent-status"],
        )

        handle = EnvironmentHandle(
            env_id="test-env",
            worktree_path=tmp_path,
            branch="tanren/demo-42",
            project="demo",
            runtime=LocalEnvironmentRuntime(preflight_result=preflight_result),
        )
        repairs = manager._preflight_repairs(handle)
        assert repairs == ["Switched branch to tanren/demo-42", "Cleared .agent-status"]

    def test_returns_empty_for_non_local_runtime(self, tmp_path: Path):
        manager = _make_manager(tmp_path)

        handle = EnvironmentHandle(
            env_id="test-env",
            worktree_path=tmp_path,
            branch="tanren/demo-42",
            project="demo",
            runtime=CustomEnvironmentRuntime(adapter="docker"),
        )
        repairs = manager._preflight_repairs(handle)
        assert repairs == []

    def test_returns_empty_when_no_preflight_result(self, tmp_path: Path):
        manager = _make_manager(tmp_path)

        handle = EnvironmentHandle(
            env_id="test-env",
            worktree_path=tmp_path,
            branch="tanren/demo-42",
            project="demo",
            runtime=LocalEnvironmentRuntime(preflight_result=None),
        )
        repairs = manager._preflight_repairs(handle)
        assert repairs == []


# ---------------------------------------------------------------------------
# build_gate_output and build_tail_output — integration-style edge cases
# ---------------------------------------------------------------------------


class TestBuildGateOutputIntegration:
    def test_multiline_output_with_trailing_whitespace(self):
        stdout = "  line 1  \n  line 2  \n"
        result = _build_gate_output(stdout, Outcome.SUCCESS)
        assert result is not None
        assert "line 1" in result
        assert "line 2" in result

    def test_single_line_output(self):
        result = _build_gate_output("only line", Outcome.FAIL)
        assert result == "only line"

    def test_exactly_at_success_limit(self):
        lines = [f"line {i}" for i in range(100)]
        result = _build_gate_output("\n".join(lines), Outcome.SUCCESS)
        assert result is not None
        assert len(result.split("\n")) == 100


class TestBuildTailOutputIntegration:
    def test_exactly_at_limit(self):
        lines = [f"line {i}" for i in range(200)]
        result = build_tail_output("\n".join(lines))
        assert result is not None
        assert len(result.split("\n")) == 200

    def test_whitespace_only_input(self):
        result = build_tail_output("   \n  \n ")
        assert result is not None


# ---------------------------------------------------------------------------
# WorkerManager initialization integration
# ---------------------------------------------------------------------------


class TestWorkerManagerInitIntegration:
    def test_emitter_from_events_db_config(self, tmp_path: Path):
        config = Config(
            ipc_dir=str(tmp_path / "ipc"),
            github_dir=str(tmp_path / "github"),
            data_dir=str(tmp_path / "data"),
            worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
            events_db=str(tmp_path / "events.db"),
        )
        manager = WorkerManager(config, execution_env=AsyncMock())
        # Should use SqliteEventEmitter, not NullEventEmitter
        assert not isinstance(manager._emitter, NullEventEmitter)

    def test_null_emitter_when_no_events_db(self, tmp_path: Path):
        config = Config(
            ipc_dir=str(tmp_path / "ipc"),
            github_dir=str(tmp_path / "github"),
            data_dir=str(tmp_path / "data"),
            worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
        )
        manager = WorkerManager(config, execution_env=AsyncMock())
        assert isinstance(manager._emitter, NullEventEmitter)

    def test_injected_emitter_takes_precedence(self, tmp_path: Path):
        config = Config(
            ipc_dir=str(tmp_path / "ipc"),
            github_dir=str(tmp_path / "github"),
            data_dir=str(tmp_path / "data"),
            worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
            events_db=str(tmp_path / "events.db"),
        )
        custom_emitter = NullEventEmitter()
        manager = WorkerManager(config, execution_env=AsyncMock(), emitter=custom_emitter)
        assert manager._emitter is custom_emitter
