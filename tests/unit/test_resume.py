"""Tests for dispatch resume logic."""

from __future__ import annotations

from typing import TYPE_CHECKING
from unittest.mock import AsyncMock

import pytest

from tanren_core.adapters.null_emitter import NullEventEmitter
from tanren_core.adapters.types import (
    EnvironmentHandle,
    LocalEnvironmentRuntime,
    PhaseResult,
)
from tanren_core.config import Config
from tanren_core.ipc import read_checkpoint, write_checkpoint
from tanren_core.manager import WorkerManager
from tanren_core.schemas import (
    Checkpoint,
    CheckpointStage,
    Cli,
    Dispatch,
    Outcome,
    Phase,
    Result,
)

if TYPE_CHECKING:
    from pathlib import Path


def _make_config(tmp_path: Path) -> Config:
    return Config(
        ipc_dir=str(tmp_path / "ipc"),
        github_dir=str(tmp_path / "github"),
        data_dir=str(tmp_path / "data"),
        worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
        roles_config_path=str(tmp_path / "roles.yml"),
    )


def _make_dispatch() -> Dispatch:
    return Dispatch(
        workflow_id="wf-test-1-1000",
        phase=Phase.DO_TASK,
        project="test",
        spec_folder="specs/test",
        branch="main",
        cli=Cli.CLAUDE,
        timeout=1800,
    )


def _make_checkpoint(
    stage: CheckpointStage = CheckpointStage.DISPATCHED,
) -> Checkpoint:
    return Checkpoint(
        workflow_id="wf-test-1-1000",
        stage=stage,
        dispatch_json=_make_dispatch().model_dump_json(),
        worktree_path="/tmp/worktree",
        dispatch_stem="1000-abc123",
        created_at="2026-01-01T00:00:00+00:00",
        updated_at="2026-01-01T00:00:00+00:00",
    )


def _make_handle() -> EnvironmentHandle:
    from pathlib import Path as _Path

    return EnvironmentHandle(
        env_id="env-1",
        worktree_path=_Path("/tmp/worktree"),
        branch="main",
        project="test",
        runtime=LocalEnvironmentRuntime(),
    )


def _make_phase_result() -> PhaseResult:
    return PhaseResult(
        outcome=Outcome.SUCCESS,
        signal="complete",
        exit_code=0,
        stdout="done",
        duration_secs=10,
        preflight_passed=True,
    )


def _make_result() -> Result:
    return Result(
        workflow_id="wf-test-1-1000",
        phase=Phase.DO_TASK,
        outcome=Outcome.SUCCESS,
        signal="complete",
        exit_code=0,
        duration_secs=10,
        gate_output=None,
        tail_output=None,
        unchecked_tasks=0,
        plan_hash="00000000",
        spec_modified=False,
    )


def _make_manager(tmp_path: Path) -> WorkerManager:
    from pathlib import Path as _Path

    config = _make_config(tmp_path)
    _Path(config.checkpoints_dir).mkdir(parents=True, exist_ok=True)
    execution_env = AsyncMock()
    execution_env.provision = AsyncMock(return_value=_make_handle())
    execution_env.execute = AsyncMock(return_value=_make_phase_result())
    execution_env.teardown = AsyncMock()
    return WorkerManager(
        config=config,
        execution_env=execution_env,
        emitter=NullEventEmitter(),
    )


class TestResumeDispatch:
    async def test_resume_nonexistent_returns_none(self, tmp_path: Path):
        manager = _make_manager(tmp_path)
        result = await manager.resume_dispatch("nonexistent")
        assert result is None

    async def test_resume_from_dispatched_runs_all_phases(
        self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
    ):
        manager = _make_manager(tmp_path)
        cp = _make_checkpoint(CheckpointStage.DISPATCHED)
        await write_checkpoint(manager._checkpoints_dir, cp)

        handle = _make_handle()
        phase_result = _make_phase_result()
        result = _make_result()
        mock_provision = AsyncMock(return_value=handle)
        mock_execute = AsyncMock(return_value=phase_result)
        mock_postprocess = AsyncMock(return_value=result)
        mock_write = AsyncMock()

        monkeypatch.setattr(manager, "_provision_phase", mock_provision)
        monkeypatch.setattr(manager, "_execute_phase", mock_execute)
        monkeypatch.setattr(manager, "_post_process_phase", mock_postprocess)
        monkeypatch.setattr(manager, "_write_result_and_nudge", mock_write)

        returned = await manager.resume_dispatch("wf-test-1-1000")
        assert returned is not None
        assert returned.outcome == Outcome.SUCCESS

        mock_provision.assert_awaited_once()
        mock_execute.assert_awaited_once()
        mock_postprocess.assert_awaited_once()
        mock_write.assert_awaited_once()

    async def test_resume_from_provisioned_skips_provision(
        self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
    ):
        manager = _make_manager(tmp_path)
        cp = _make_checkpoint(CheckpointStage.PROVISIONED)
        await write_checkpoint(manager._checkpoints_dir, cp)

        handle = _make_handle()
        phase_result = _make_phase_result()
        result = _make_result()
        mock_provision = AsyncMock(return_value=handle)
        mock_execute = AsyncMock(return_value=phase_result)
        mock_postprocess = AsyncMock(return_value=result)

        monkeypatch.setattr(manager, "_provision_phase", mock_provision)
        monkeypatch.setattr(manager, "_execute_phase", mock_execute)
        monkeypatch.setattr(manager, "_post_process_phase", mock_postprocess)
        monkeypatch.setattr(
            manager, "_reconstruct_handle_for_resume", AsyncMock(return_value=handle)
        )
        monkeypatch.setattr(manager, "_write_result_and_nudge", AsyncMock())

        returned = await manager.resume_dispatch("wf-test-1-1000")
        assert returned is not None

        mock_provision.assert_not_awaited()
        mock_execute.assert_awaited_once()
        mock_postprocess.assert_awaited_once()

    async def test_resume_from_executed_skips_execute(
        self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
    ):
        manager = _make_manager(tmp_path)
        cp = _make_checkpoint(CheckpointStage.EXECUTED)
        cp.phase_result_json = _make_phase_result().model_dump_json()
        await write_checkpoint(manager._checkpoints_dir, cp)

        handle = _make_handle()
        result = _make_result()
        mock_provision = AsyncMock(return_value=handle)
        mock_execute = AsyncMock()
        mock_postprocess = AsyncMock(return_value=result)

        monkeypatch.setattr(manager, "_provision_phase", mock_provision)
        monkeypatch.setattr(manager, "_execute_phase", mock_execute)
        monkeypatch.setattr(manager, "_post_process_phase", mock_postprocess)
        monkeypatch.setattr(
            manager, "_reconstruct_handle_for_resume", AsyncMock(return_value=handle)
        )
        monkeypatch.setattr(manager, "_write_result_and_nudge", AsyncMock())

        returned = await manager.resume_dispatch("wf-test-1-1000")
        assert returned is not None

        mock_provision.assert_not_awaited()
        mock_execute.assert_not_awaited()
        mock_postprocess.assert_awaited_once()

    async def test_resume_increments_retry_count(
        self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
    ):
        manager = _make_manager(tmp_path)
        cp = _make_checkpoint(CheckpointStage.PROVISIONED)
        cp.retry_count = 2
        await write_checkpoint(manager._checkpoints_dir, cp)

        handle = _make_handle()
        phase_result = _make_phase_result()
        result = _make_result()
        mock_execute = AsyncMock(return_value=phase_result)

        monkeypatch.setattr(manager, "_provision_phase", AsyncMock())
        monkeypatch.setattr(manager, "_execute_phase", mock_execute)
        monkeypatch.setattr(manager, "_post_process_phase", AsyncMock(return_value=result))
        monkeypatch.setattr(
            manager, "_reconstruct_handle_for_resume", AsyncMock(return_value=handle)
        )
        monkeypatch.setattr(manager, "_write_result_and_nudge", AsyncMock())

        await manager.resume_dispatch("wf-test-1-1000")
        mock_execute.assert_awaited_once()

    async def test_resume_stores_error_on_failure(
        self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
    ):
        manager = _make_manager(tmp_path)
        cp = _make_checkpoint(CheckpointStage.PROVISIONED)
        await write_checkpoint(manager._checkpoints_dir, cp)

        monkeypatch.setattr(
            manager, "_reconstruct_handle_for_resume", AsyncMock(side_effect=ValueError("VM gone"))
        )

        with pytest.raises(ValueError, match="VM gone"):
            await manager.resume_dispatch("wf-test-1-1000")

        updated = await read_checkpoint(manager._checkpoints_dir, "wf-test-1-1000")
        assert updated is not None
        assert updated.last_error == "VM gone"
        assert updated.failure_count == 1

    async def test_resume_teardown_always_called(
        self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
    ):
        manager = _make_manager(tmp_path)
        cp = _make_checkpoint(CheckpointStage.PROVISIONED)
        await write_checkpoint(manager._checkpoints_dir, cp)

        handle = _make_handle()
        monkeypatch.setattr(
            manager, "_reconstruct_handle_for_resume", AsyncMock(return_value=handle)
        )
        monkeypatch.setattr(manager, "_execute_phase", AsyncMock(side_effect=RuntimeError("boom")))

        with pytest.raises(RuntimeError, match="boom"):
            await manager.resume_dispatch("wf-test-1-1000")

        teardown_mock: AsyncMock = manager._execution_env.teardown  # type: ignore[assignment]
        teardown_mock.assert_awaited_once_with(handle)

    async def test_resume_checkpoint_deleted_on_success(
        self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
    ):
        manager = _make_manager(tmp_path)
        cp = _make_checkpoint(CheckpointStage.PROVISIONED)
        await write_checkpoint(manager._checkpoints_dir, cp)

        handle = _make_handle()
        phase_result = _make_phase_result()
        result = _make_result()

        monkeypatch.setattr(manager, "_provision_phase", AsyncMock())
        monkeypatch.setattr(manager, "_execute_phase", AsyncMock(return_value=phase_result))
        monkeypatch.setattr(manager, "_post_process_phase", AsyncMock(return_value=result))
        monkeypatch.setattr(
            manager, "_reconstruct_handle_for_resume", AsyncMock(return_value=handle)
        )
        monkeypatch.setattr(manager, "_write_result_and_nudge", AsyncMock())

        await manager.resume_dispatch("wf-test-1-1000")

        loaded = await read_checkpoint(manager._checkpoints_dir, "wf-test-1-1000")
        assert loaded is None


class TestReconstructHandle:
    async def test_local_reconstruction(self, tmp_path: Path):
        from pathlib import Path as _Path

        manager = _make_manager(tmp_path)
        cp = _make_checkpoint(CheckpointStage.PROVISIONED)
        dispatch = _make_dispatch()

        handle = await manager._reconstruct_handle_for_resume(cp, dispatch)
        assert handle.runtime.kind == "local"
        assert handle.worktree_path == _Path("/tmp/worktree")
        assert handle.branch == "main"
        assert handle.project == "test"

    async def test_remote_vm_released_raises(self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
        manager = _make_manager(tmp_path)
        cp = _make_checkpoint(CheckpointStage.PROVISIONED)
        cp.vm_id = "vm-gone"
        dispatch = _make_dispatch()

        store = AsyncMock()
        store.get_assignment = AsyncMock(return_value=None)
        manager._remote_state_store = store  # dynamically set in remote mode

        with pytest.raises(ValueError, match="no longer active"):
            await manager._reconstruct_handle_for_resume(cp, dispatch)
