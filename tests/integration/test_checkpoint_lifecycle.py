"""Integration tests for checkpoint lifecycle."""

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
from tanren_core.env.environment_schema import EnvironmentProfile
from tanren_core.ipc import list_checkpoints, read_checkpoint, write_checkpoint
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

DEFAULT_PROFILE = EnvironmentProfile(name="default")

if TYPE_CHECKING:
    from pathlib import Path


def _config(tmp_path: Path) -> Config:
    return Config(
        ipc_dir=str(tmp_path / "ipc"),
        github_dir=str(tmp_path / "github"),
        data_dir=str(tmp_path / "data"),
        worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
        roles_config_path=str(tmp_path / "roles.yml"),
    )


def _dispatch() -> Dispatch:
    return Dispatch(
        workflow_id="wf-test-1-1000",
        phase=Phase.DO_TASK,
        project="test",
        spec_folder="specs/test",
        branch="main",
        cli=Cli.CLAUDE,
        timeout=1800,
        resolved_profile=DEFAULT_PROFILE,
    )


def _checkpoint(stage: CheckpointStage = CheckpointStage.DISPATCHED) -> Checkpoint:
    return Checkpoint(
        workflow_id="wf-test-1-1000",
        stage=stage,
        dispatch_json=_dispatch().model_dump_json(),
        worktree_path="/tmp/worktree",
        dispatch_stem="1000-abc123",
        created_at="2026-01-01T00:00:00+00:00",
        updated_at="2026-01-01T00:00:00+00:00",
    )


def _handle() -> EnvironmentHandle:
    from pathlib import Path as _Path

    return EnvironmentHandle(
        env_id="env-1",
        worktree_path=_Path("/tmp/worktree"),
        branch="main",
        project="test",
        runtime=LocalEnvironmentRuntime(),
    )


def _phase_result() -> PhaseResult:
    return PhaseResult(
        outcome=Outcome.SUCCESS,
        signal="complete",
        exit_code=0,
        stdout="done",
        duration_secs=10,
        preflight_passed=True,
    )


def _result() -> Result:
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


def _manager(tmp_path: Path) -> WorkerManager:
    from pathlib import Path as _Path

    config = _config(tmp_path)
    _Path(config.checkpoints_dir).mkdir(parents=True, exist_ok=True)
    return WorkerManager(
        config=config,
        execution_env=AsyncMock(),
        emitter=NullEventEmitter(),
    )


class TestCheckpointLifecycle:
    async def test_resume_full_lifecycle_deletes_checkpoint(
        self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
    ):
        """Resume from DISPATCHED, complete all phases, checkpoint deleted."""
        manager = _manager(tmp_path)
        cp = _checkpoint(CheckpointStage.DISPATCHED)
        await write_checkpoint(manager._checkpoints_dir, cp)

        loaded = await read_checkpoint(manager._checkpoints_dir, "wf-test-1-1000")
        assert loaded is not None
        assert loaded.stage == CheckpointStage.DISPATCHED

        monkeypatch.setattr(manager, "_provision_phase", AsyncMock(return_value=_handle()))
        monkeypatch.setattr(manager, "_execute_phase", AsyncMock(return_value=_phase_result()))
        monkeypatch.setattr(manager, "_post_process_phase", AsyncMock(return_value=_result()))
        monkeypatch.setattr(manager, "_write_result_and_nudge", AsyncMock())

        result = await manager.resume_dispatch("wf-test-1-1000")
        assert result is not None
        assert result.outcome == Outcome.SUCCESS

        loaded = await read_checkpoint(manager._checkpoints_dir, "wf-test-1-1000")
        assert loaded is None

    async def test_resume_from_provisioned_skips_provision(
        self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
    ):
        """Resume from PROVISIONED skips provision, still completes."""
        manager = _manager(tmp_path)
        cp = _checkpoint(CheckpointStage.PROVISIONED)
        await write_checkpoint(manager._checkpoints_dir, cp)

        mock_provision = AsyncMock()
        mock_execute = AsyncMock(return_value=_phase_result())
        mock_postprocess = AsyncMock(return_value=_result())

        monkeypatch.setattr(manager, "_provision_phase", mock_provision)
        monkeypatch.setattr(manager, "_execute_phase", mock_execute)
        monkeypatch.setattr(manager, "_post_process_phase", mock_postprocess)
        monkeypatch.setattr(
            manager, "_reconstruct_handle_for_resume", AsyncMock(return_value=_handle())
        )
        monkeypatch.setattr(manager, "_write_result_and_nudge", AsyncMock())

        result = await manager.resume_dispatch("wf-test-1-1000")
        assert result is not None

        mock_provision.assert_not_awaited()
        mock_execute.assert_awaited_once()
        mock_postprocess.assert_awaited_once()

        assert await read_checkpoint(manager._checkpoints_dir, "wf-test-1-1000") is None

    async def test_failure_preserves_checkpoint_with_error(
        self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
    ):
        """Failure during resume preserves checkpoint with error info."""
        manager = _manager(tmp_path)
        cp = _checkpoint(CheckpointStage.PROVISIONED)
        await write_checkpoint(manager._checkpoints_dir, cp)

        monkeypatch.setattr(
            manager, "_reconstruct_handle_for_resume", AsyncMock(return_value=_handle())
        )
        monkeypatch.setattr(
            manager, "_execute_phase", AsyncMock(side_effect=RuntimeError("network timeout"))
        )

        with pytest.raises(RuntimeError, match="network timeout"):
            await manager.resume_dispatch("wf-test-1-1000")

        updated = await read_checkpoint(manager._checkpoints_dir, "wf-test-1-1000")
        assert updated is not None
        assert updated.last_error == "network timeout"
        assert updated.failure_count == 1
        assert updated.retry_count == 1

    async def test_retry_after_failure_succeeds(
        self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
    ):
        """After a failed resume, a second resume succeeds and deletes checkpoint."""
        manager = _manager(tmp_path)
        cp = _checkpoint(CheckpointStage.PROVISIONED)
        await write_checkpoint(manager._checkpoints_dir, cp)

        # First attempt: fail
        monkeypatch.setattr(
            manager, "_reconstruct_handle_for_resume", AsyncMock(return_value=_handle())
        )
        monkeypatch.setattr(manager, "_execute_phase", AsyncMock(side_effect=RuntimeError("oops")))

        with pytest.raises(RuntimeError):
            await manager.resume_dispatch("wf-test-1-1000")

        updated = await read_checkpoint(manager._checkpoints_dir, "wf-test-1-1000")
        assert updated is not None
        assert updated.failure_count == 1

        # Second attempt: succeed
        monkeypatch.setattr(manager, "_execute_phase", AsyncMock(return_value=_phase_result()))
        monkeypatch.setattr(manager, "_post_process_phase", AsyncMock(return_value=_result()))
        monkeypatch.setattr(manager, "_write_result_and_nudge", AsyncMock())

        result = await manager.resume_dispatch("wf-test-1-1000")
        assert result is not None
        assert result.outcome == Outcome.SUCCESS

        assert await read_checkpoint(manager._checkpoints_dir, "wf-test-1-1000") is None

    async def test_multiple_checkpoints_listed(self, tmp_path: Path):
        """Multiple checkpoints can be listed."""
        manager = _manager(tmp_path)
        for i in range(3):
            cp = Checkpoint(
                workflow_id=f"wf-test-{i}-1000",
                stage=CheckpointStage.PROVISIONED,
                dispatch_json=_dispatch().model_dump_json(),
                worktree_path="/tmp/worktree",
                created_at="2026-01-01T00:00:00+00:00",
                updated_at="2026-01-01T00:00:00+00:00",
            )
            await write_checkpoint(manager._checkpoints_dir, cp)

        entries = await list_checkpoints(manager._checkpoints_dir)
        assert len(entries) == 3
