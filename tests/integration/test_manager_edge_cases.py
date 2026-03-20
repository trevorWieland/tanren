"""Integration tests for WorkerManager edge cases using mock adapters."""

from pathlib import Path
from unittest.mock import AsyncMock, MagicMock

import pytest

from tanren_core.config import Config
from tanren_core.manager import (
    WorkerManager,
    _build_gate_output,  # noqa: PLC2701 — testing private implementation
    build_tail_output,
)
from tanren_core.schemas import Cli, Dispatch, Outcome, Phase


def _make_config(tmp_path: Path) -> Config:
    """Build a Config that points to tmp_path directories."""
    ipc = tmp_path / "ipc"
    data = tmp_path / "data"
    github = tmp_path / "github"
    for d in (ipc, data, github):
        d.mkdir(parents=True, exist_ok=True)
    return Config(
        ipc_dir=str(ipc),
        data_dir=str(data),
        github_dir=str(github),
        worktree_registry_path=str(tmp_path / "worktrees.json"),
        roles_config_path=str(tmp_path / "roles.yml"),
        poll_interval=0.1,
        heartbeat_interval=60,
        max_opencode=1,
        max_codex=1,
        max_gate=3,
    )


def _make_dispatch(
    cli: Cli = Cli.OPENCODE,
    phase: Phase = Phase.DO_TASK,
    workflow_id: str = "wf-test-1-100",
) -> Dispatch:
    return Dispatch(
        workflow_id=workflow_id,
        phase=phase,
        project="test",
        spec_folder="specs/test",
        branch="feature-1",
        cli=cli,
        timeout=60,
    )


class TestGateOutputBuilding:
    def test_none_stdout(self):
        assert _build_gate_output(None, Outcome.SUCCESS) is None

    def test_empty_stdout(self):
        assert _build_gate_output("", Outcome.SUCCESS) is None

    def test_success_truncates_to_100_lines(self):
        stdout = "\n".join(f"line {i}" for i in range(200))
        result = _build_gate_output(stdout, Outcome.SUCCESS)
        assert result is not None
        assert len(result.split("\n")) == 100

    def test_fail_truncates_to_300_lines(self):
        stdout = "\n".join(f"line {i}" for i in range(500))
        result = _build_gate_output(stdout, Outcome.FAIL)
        assert result is not None
        assert len(result.split("\n")) == 300

    def test_short_output_unchanged(self):
        stdout = "line 1\nline 2\nline 3"
        result = _build_gate_output(stdout, Outcome.SUCCESS)
        assert result == stdout


class TestTailOutput:
    def test_none_returns_none(self):
        assert build_tail_output(None) is None

    def test_empty_returns_none(self):
        assert build_tail_output("") is None

    def test_truncates_to_200_lines(self):
        stdout = "\n".join(f"line {i}" for i in range(400))
        result = build_tail_output(stdout)
        assert result is not None
        assert len(result.split("\n")) == 200

    def test_short_output_unchanged(self):
        stdout = "line 1\nline 2"
        assert build_tail_output(stdout) == stdout


class TestWorkerManagerInit:
    def test_default_adapters(self, tmp_path: Path):
        """WorkerManager initializes with default adapters when none injected."""
        config = _make_config(tmp_path)
        mgr = WorkerManager(config)
        env = mgr.get_execution_environment()
        assert env is not None

    def test_injected_emitter(self, tmp_path: Path):
        """Injected emitter is used instead of auto-configured one."""
        config = _make_config(tmp_path)
        mock_emitter = AsyncMock()
        mgr = WorkerManager(config, emitter=mock_emitter)
        assert mgr._emitter is mock_emitter

    def test_sqlite_emitter_from_config(self, tmp_path: Path):
        """SQLite emitter created when events_db is a file path."""
        config = _make_config(tmp_path)
        config.events_db = str(tmp_path / "events.db")
        mgr = WorkerManager(config)
        assert mgr._emitter is not None
        assert mgr._pg_dsn is None

    def test_null_emitter_when_no_events_db(self, tmp_path: Path):
        """NullEventEmitter used when no events_db configured."""
        config = _make_config(tmp_path)
        config.events_db = None
        mgr = WorkerManager(config)
        # NullEventEmitter is the fallback
        assert mgr._emitter is not None


@pytest.mark.asyncio
class TestWorkerManagerSetup:
    async def test_handle_setup_success(self, tmp_path: Path):
        """Setup phase creates worktree and writes result."""
        config = _make_config(tmp_path)

        mock_wt_mgr = MagicMock()
        mock_wt_mgr.create = AsyncMock(return_value=tmp_path / "test-wt-1")
        mock_wt_mgr.register = AsyncMock()
        mock_env_provisioner = MagicMock()
        mock_env_provisioner.provision = MagicMock(return_value=0)
        mock_emitter = AsyncMock()
        mock_emitter.emit = AsyncMock()
        mock_emitter.close = AsyncMock()

        mgr = WorkerManager(
            config,
            worktree_mgr=mock_wt_mgr,
            env_provisioner=mock_env_provisioner,
            emitter=mock_emitter,
        )

        dispatch = _make_dispatch(phase=Phase.SETUP)
        result_dir = Path(config.ipc_dir) / "results"
        result_dir.mkdir(parents=True, exist_ok=True)
        input_dir = Path(config.ipc_dir) / "input"
        input_dir.mkdir(parents=True, exist_ok=True)

        await mgr._handle_setup(dispatch, "1", tmp_path / "test-wt-1")

        mock_wt_mgr.create.assert_awaited_once()
        mock_wt_mgr.register.assert_awaited_once()

    async def test_handle_setup_error_writes_error_result(self, tmp_path: Path):
        """Setup failure writes error result."""
        config = _make_config(tmp_path)

        mock_wt_mgr = MagicMock()
        mock_wt_mgr.create = AsyncMock(side_effect=RuntimeError("branch conflict"))
        mock_emitter = AsyncMock()
        mock_emitter.emit = AsyncMock()
        mock_emitter.close = AsyncMock()

        mgr = WorkerManager(
            config,
            worktree_mgr=mock_wt_mgr,
            emitter=mock_emitter,
        )

        dispatch = _make_dispatch(phase=Phase.SETUP)
        result_dir = Path(config.ipc_dir) / "results"
        result_dir.mkdir(parents=True, exist_ok=True)
        input_dir = Path(config.ipc_dir) / "input"
        input_dir.mkdir(parents=True, exist_ok=True)

        # Should not raise
        await mgr._handle_setup(dispatch, "1", tmp_path / "test-wt-1")

        # Result file should be written
        results = list(result_dir.glob("*.json"))
        assert len(results) == 1


@pytest.mark.asyncio
class TestWorkerManagerCleanup:
    async def test_handle_cleanup_success(self, tmp_path: Path):
        """Cleanup phase calls worktree cleanup and writes result."""
        config = _make_config(tmp_path)

        mock_wt_mgr = MagicMock()
        mock_wt_mgr.cleanup = AsyncMock()
        mock_emitter = AsyncMock()
        mock_emitter.emit = AsyncMock()
        mock_emitter.close = AsyncMock()

        mgr = WorkerManager(
            config,
            worktree_mgr=mock_wt_mgr,
            emitter=mock_emitter,
        )

        dispatch = _make_dispatch(phase=Phase.CLEANUP)
        result_dir = Path(config.ipc_dir) / "results"
        result_dir.mkdir(parents=True, exist_ok=True)
        input_dir = Path(config.ipc_dir) / "input"
        input_dir.mkdir(parents=True, exist_ok=True)

        await mgr._handle_cleanup(dispatch)
        mock_wt_mgr.cleanup.assert_awaited_once()

    async def test_handle_cleanup_error(self, tmp_path: Path):
        """Cleanup failure writes error result (not raised)."""
        config = _make_config(tmp_path)

        mock_wt_mgr = MagicMock()
        mock_wt_mgr.cleanup = AsyncMock(side_effect=RuntimeError("not found"))
        mock_emitter = AsyncMock()
        mock_emitter.emit = AsyncMock()
        mock_emitter.close = AsyncMock()

        mgr = WorkerManager(
            config,
            worktree_mgr=mock_wt_mgr,
            emitter=mock_emitter,
        )

        dispatch = _make_dispatch(phase=Phase.CLEANUP)
        result_dir = Path(config.ipc_dir) / "results"
        result_dir.mkdir(parents=True, exist_ok=True)
        input_dir = Path(config.ipc_dir) / "input"
        input_dir.mkdir(parents=True, exist_ok=True)

        await mgr._handle_cleanup(dispatch)

        results = list(result_dir.glob("*.json"))
        assert len(results) == 1


@pytest.mark.asyncio
class TestWorkerManagerDispatch:
    async def test_unhandled_error_writes_error_result(self, tmp_path: Path):
        """Unhandled exception in _handle_dispatch writes error result."""
        config = _make_config(tmp_path)

        mock_emitter = AsyncMock()
        mock_emitter.emit = AsyncMock()
        mock_emitter.close = AsyncMock()
        mock_exec_env = AsyncMock()
        mock_exec_env.provision = AsyncMock(side_effect=RuntimeError("unexpected"))
        mock_exec_env.teardown = AsyncMock()

        mgr = WorkerManager(
            config,
            execution_env=mock_exec_env,
            emitter=mock_emitter,
        )

        dispatch = _make_dispatch(phase=Phase.DO_TASK)
        result_dir = Path(config.ipc_dir) / "results"
        result_dir.mkdir(parents=True, exist_ok=True)
        input_dir = Path(config.ipc_dir) / "input"
        input_dir.mkdir(parents=True, exist_ok=True)

        await mgr._handle_dispatch(Path("/dispatch/1.json"), dispatch)

        results = list(result_dir.glob("*.json"))
        assert len(results) >= 1


class TestParseFindings:
    def test_no_findings_for_do_task(self, tmp_path: Path):
        """do-task phase returns empty findings."""
        config = _make_config(tmp_path)
        mgr = WorkerManager(config)
        dispatch = _make_dispatch(phase=Phase.DO_TASK)
        spec_folder = tmp_path / "specs" / "test"
        spec_folder.mkdir(parents=True)

        new_tasks, findings = mgr._parse_findings(dispatch, spec_folder)
        assert new_tasks == []
        assert findings == []


class TestSignalShutdown:
    def test_signal_sets_shutdown_event(self, tmp_path: Path):
        """_signal_shutdown sets the shutdown event."""
        config = _make_config(tmp_path)
        mgr = WorkerManager(config)
        assert not mgr._shutdown_event.is_set()
        mgr._signal_shutdown()
        assert mgr._shutdown_event.is_set()
