"""Tests for manager module."""

from pathlib import Path

from worker_manager.config import Config
from worker_manager.manager import WorkerManager


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
