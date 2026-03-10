"""Tests for manager module."""

from pathlib import Path

from worker_manager.config import Config
from worker_manager.manager import (
    _GATE_OUTPUT_LINES_FAIL,
    _GATE_OUTPUT_LINES_SUCCESS,
    _TAIL_OUTPUT_LINES,
    WorkerManager,
    _build_gate_output,
    _build_tail_output,
)
from worker_manager.schemas import Outcome


class TestWorkerManagerInit:
    def test_creates_with_config(self, tmp_path: Path):
        config = Config(
            ipc_dir=str(tmp_path / "ipc"),
            github_dir=str(tmp_path / "github"),
            data_dir=str(tmp_path / "data"),
            worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
        )
        manager = WorkerManager(config)
        assert manager._config == config

    def test_directories_derived_from_config(self, tmp_path: Path):
        ipc = tmp_path / "ipc"
        config = Config(
            ipc_dir=str(ipc),
            github_dir=str(tmp_path / "github"),
            data_dir=str(tmp_path / "data"),
            worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
        )
        manager = WorkerManager(config)
        assert manager._dispatch_dir == ipc / "dispatch"
        assert manager._results_dir == ipc / "results"
        assert manager._in_progress_dir == ipc / "in-progress"
        assert manager._input_dir == ipc / "input"


class TestBuildGateOutput:
    def test_none_when_stdout_is_none(self):
        assert _build_gate_output(None, Outcome.SUCCESS) is None

    def test_none_when_stdout_is_empty(self):
        assert _build_gate_output("", Outcome.FAIL) is None

    def test_success_truncates_to_100_lines(self):
        lines = [f"line {i}" for i in range(200)]
        result = _build_gate_output("\n".join(lines), Outcome.SUCCESS)
        result_lines = result.split("\n")
        assert len(result_lines) == _GATE_OUTPUT_LINES_SUCCESS
        assert result_lines[0] == "line 100"
        assert result_lines[-1] == "line 199"

    def test_fail_truncates_to_300_lines(self):
        lines = [f"line {i}" for i in range(500)]
        result = _build_gate_output("\n".join(lines), Outcome.FAIL)
        result_lines = result.split("\n")
        assert len(result_lines) == _GATE_OUTPUT_LINES_FAIL
        assert result_lines[0] == "line 200"
        assert result_lines[-1] == "line 499"

    def test_short_output_returned_intact(self):
        result = _build_gate_output("hello\nworld", Outcome.SUCCESS)
        assert result == "hello\nworld"

    def test_error_uses_fail_limit(self):
        lines = [f"line {i}" for i in range(500)]
        result = _build_gate_output("\n".join(lines), Outcome.ERROR)
        result_lines = result.split("\n")
        assert len(result_lines) == _GATE_OUTPUT_LINES_FAIL


class TestBuildTailOutput:
    def test_none_when_stdout_is_none(self):
        assert _build_tail_output(None) is None

    def test_none_when_stdout_is_empty(self):
        assert _build_tail_output("") is None

    def test_truncates_to_200_lines(self):
        lines = [f"line {i}" for i in range(400)]
        result = _build_tail_output("\n".join(lines))
        result_lines = result.split("\n")
        assert len(result_lines) == _TAIL_OUTPUT_LINES
        assert result_lines[0] == "line 200"
        assert result_lines[-1] == "line 399"

    def test_short_output_returned_intact(self):
        result = _build_tail_output("one\ntwo\nthree")
        assert result == "one\ntwo\nthree"
