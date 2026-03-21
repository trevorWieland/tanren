"""Tests for checkpoint schema and I/O."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest
from pydantic import ValidationError

if TYPE_CHECKING:
    from pathlib import Path

from tanren_core.env.environment_schema import EnvironmentProfile
from tanren_core.ipc import (
    delete_checkpoint,
    list_checkpoints,
    read_checkpoint,
    write_checkpoint,
)
from tanren_core.schemas import (
    Checkpoint,
    CheckpointStage,
    Cli,
    Dispatch,
    Phase,
)

DEFAULT_PROFILE = EnvironmentProfile(name="default")


def _make_dispatch() -> Dispatch:
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


def _make_checkpoint(workflow_id: str = "wf-test-1-1000") -> Checkpoint:
    return Checkpoint(
        workflow_id=workflow_id,
        stage=CheckpointStage.DISPATCHED,
        dispatch_json=_make_dispatch().model_dump_json(),
        worktree_path="/tmp/worktree",
        dispatch_stem="1000-abc123",
        created_at="2026-01-01T00:00:00+00:00",
        updated_at="2026-01-01T00:00:00+00:00",
    )


class TestCheckpointModel:
    def test_roundtrip_serialization(self):
        cp = _make_checkpoint()
        json_str = cp.model_dump_json()
        cp2 = Checkpoint.model_validate_json(json_str)
        assert cp == cp2

    def test_stage_enum_values(self):
        assert CheckpointStage.DISPATCHED == "dispatched"
        assert CheckpointStage.PROVISIONED == "provisioned"
        assert CheckpointStage.EXECUTED == "executed"
        assert CheckpointStage.POST_PROCESSED == "post_processed"

    def test_extra_forbid_rejects_unknown(self):
        data = _make_checkpoint().model_dump()
        data["unknown_field"] = "bad"
        with pytest.raises(ValidationError):
            Checkpoint.model_validate(data)

    def test_dispatch_preserved_in_json(self):
        cp = _make_checkpoint()
        dispatch = Dispatch.model_validate_json(cp.dispatch_json)
        assert dispatch.workflow_id == "wf-test-1-1000"
        assert dispatch.phase == Phase.DO_TASK

    def test_default_failure_fields(self):
        cp = _make_checkpoint()
        assert cp.retry_count == 0
        assert cp.failure_count == 0
        assert cp.last_error is None
        assert cp.vm_id is None
        assert cp.phase_result_json is None


class TestCheckpointIO:
    async def test_write_and_read(self, tmp_path: Path):
        cp = _make_checkpoint()
        path = await write_checkpoint(tmp_path, cp)
        assert path.exists()
        assert path.name == "wf-test-1-1000.json"

        loaded = await read_checkpoint(tmp_path, "wf-test-1-1000")
        assert loaded is not None
        assert loaded == cp

    async def test_write_overwrites_same_workflow(self, tmp_path: Path):
        cp = _make_checkpoint()
        await write_checkpoint(tmp_path, cp)

        cp.stage = CheckpointStage.PROVISIONED
        cp.updated_at = "2026-01-01T01:00:00+00:00"
        await write_checkpoint(tmp_path, cp)

        loaded = await read_checkpoint(tmp_path, "wf-test-1-1000")
        assert loaded is not None
        assert loaded.stage == CheckpointStage.PROVISIONED

        # Only one file should exist
        all_checkpoints = await list_checkpoints(tmp_path)
        assert len(all_checkpoints) == 1

    async def test_read_nonexistent_returns_none(self, tmp_path: Path):
        result = await read_checkpoint(tmp_path, "nonexistent")
        assert result is None

    async def test_delete_checkpoint(self, tmp_path: Path):
        cp = _make_checkpoint()
        await write_checkpoint(tmp_path, cp)
        await delete_checkpoint(tmp_path, "wf-test-1-1000")
        result = await read_checkpoint(tmp_path, "wf-test-1-1000")
        assert result is None

    async def test_delete_nonexistent_no_error(self, tmp_path: Path):
        # Should not raise
        await delete_checkpoint(tmp_path, "nonexistent")

    async def test_list_empty_dir(self, tmp_path: Path):
        result = await list_checkpoints(tmp_path)
        assert result == []

    async def test_list_nonexistent_dir(self, tmp_path: Path):
        result = await list_checkpoints(tmp_path / "nonexistent")
        assert result == []

    async def test_list_multiple(self, tmp_path: Path):
        for i in range(3):
            cp = _make_checkpoint(f"wf-test-{i}-1000")
            await write_checkpoint(tmp_path, cp)
        result = await list_checkpoints(tmp_path)
        assert len(result) == 3

    async def test_list_skips_invalid_json(self, tmp_path: Path):
        cp = _make_checkpoint()
        await write_checkpoint(tmp_path, cp)
        # Write corrupt file
        (tmp_path / "corrupt.json").write_text("not json")
        result = await list_checkpoints(tmp_path)
        assert len(result) == 1
        assert result[0].workflow_id == "wf-test-1-1000"


class TestCheckpointConfig:
    def test_checkpoints_dir_derived_from_data_dir(self, tmp_path: Path):
        from tanren_core.config import Config

        config = Config(
            ipc_dir=str(tmp_path / "ipc"),
            github_dir=str(tmp_path / "github"),
            data_dir=str(tmp_path / "data"),
            worktree_registry_path=str(tmp_path / "data" / "worktrees.json"),
            roles_config_path=str(tmp_path / "roles.yml"),
        )
        assert config.checkpoints_dir == str(tmp_path / "data" / "checkpoints")
