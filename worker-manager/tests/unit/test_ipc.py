"""Tests for ipc module."""

import json
import re
from pathlib import Path

import pytest

from worker_manager.ipc import (
    atomic_write,
    delete_file,
    generate_filename,
    scan_dispatch_dir,
    write_nudge,
    write_result,
)
from worker_manager.schemas import Dispatch, Nudge, Outcome, Phase, Result


class TestGenerateFilename:
    def test_format(self):
        name = generate_filename()
        assert re.match(r"^\d+-[0-9a-f]{6}\.json$", name)

    def test_unique(self):
        names = {generate_filename() for _ in range(100)}
        assert len(names) == 100


class TestAtomicWrite:
    @pytest.mark.asyncio
    async def test_writes_content(self, tmp_path: Path):
        target = tmp_path / "test.json"
        await atomic_write(target, '{"key": "value"}')
        assert target.read_text() == '{"key": "value"}'

    @pytest.mark.asyncio
    async def test_no_tmp_file_remains(self, tmp_path: Path):
        target = tmp_path / "test.json"
        await atomic_write(target, "content")
        assert not (tmp_path / "test.tmp").exists()


class TestScanDispatchDir:
    @pytest.mark.asyncio
    async def test_empty_dir(self, tmp_path: Path):
        dispatch_dir = tmp_path / "dispatch"
        dispatch_dir.mkdir()
        result = await scan_dispatch_dir(dispatch_dir)
        assert result == []

    @pytest.mark.asyncio
    async def test_nonexistent_dir(self, tmp_path: Path):
        result = await scan_dispatch_dir(tmp_path / "nonexistent")
        assert result == []

    @pytest.mark.asyncio
    async def test_ignores_tmp_files(self, tmp_path: Path):
        dispatch_dir = tmp_path / "dispatch"
        dispatch_dir.mkdir()
        (dispatch_dir / "123-abc123.tmp").write_text("{}")
        result = await scan_dispatch_dir(dispatch_dir)
        assert result == []

    @pytest.mark.asyncio
    async def test_parses_valid_dispatch(self, tmp_path: Path):
        dispatch_dir = tmp_path / "dispatch"
        dispatch_dir.mkdir()
        dispatch = Dispatch(
            workflow_id="wf-rentl-144-1741359600",
            phase=Phase.GATE,
            project="rentl",
            spec_folder="tanren/specs/test",
            branch="main",
            cli="bash",
            model=None,
            gate_cmd="make check",
            context=None,
            timeout=300,
        )
        file_path = dispatch_dir / "1741359700123-a3f2b8.json"
        file_path.write_text(dispatch.model_dump_json())
        result = await scan_dispatch_dir(dispatch_dir)
        assert len(result) == 1
        assert result[0][1].workflow_id == "wf-rentl-144-1741359600"

    @pytest.mark.asyncio
    async def test_sorted_by_filename(self, tmp_path: Path):
        dispatch_dir = tmp_path / "dispatch"
        dispatch_dir.mkdir()
        for ts in ["1741359700123", "1741359700100", "1741359700200"]:
            d = Dispatch(
                workflow_id=f"wf-rentl-{ts}-1741359600",
                phase=Phase.GATE,
                project="rentl",
                spec_folder="tanren/specs/test",
                branch="main",
                cli="bash",
                model=None,
                gate_cmd="make check",
                context=None,
                timeout=300,
            )
            (dispatch_dir / f"{ts}-aaaaaa.json").write_text(d.model_dump_json())
        result = await scan_dispatch_dir(dispatch_dir)
        assert len(result) == 3
        # Should be sorted chronologically
        wids = [r[1].workflow_id for r in result]
        assert wids[0].endswith("1741359700100-1741359600")

    @pytest.mark.asyncio
    async def test_skips_invalid_json(self, tmp_path: Path):
        dispatch_dir = tmp_path / "dispatch"
        dispatch_dir.mkdir()
        (dispatch_dir / "123-abc123.json").write_text("not json")
        result = await scan_dispatch_dir(dispatch_dir)
        assert result == []


class TestWriteResult:
    @pytest.mark.asyncio
    async def test_writes_valid_json(self, tmp_path: Path):
        result = Result(
            workflow_id="wf-rentl-144-1741359600",
            phase=Phase.GATE,
            outcome=Outcome.SUCCESS,
            signal=None,
            exit_code=0,
            duration_secs=87,
            gate_output=None,
            tail_output=None,
            unchecked_tasks=2,
            plan_hash="a3f2b8c1",
            spec_modified=False,
        )
        path = await write_result(tmp_path, result)
        assert path.exists()
        data = json.loads(path.read_text())
        assert data["outcome"] == "success"


class TestWriteNudge:
    @pytest.mark.asyncio
    async def test_writes_nudge_in_message_envelope(self, tmp_path: Path):
        nudge = Nudge(workflow_id="wf-rentl-144-1741359600")
        path = await write_nudge(tmp_path, nudge)
        assert path.exists()
        envelope = json.loads(path.read_text())
        assert envelope["type"] == "message"
        assert "text" in envelope
        inner = json.loads(envelope["text"])
        assert inner["type"] == "workflow_result"
        assert inner["workflow_id"] == "wf-rentl-144-1741359600"


class TestDeleteFile:
    @pytest.mark.asyncio
    async def test_deletes_existing(self, tmp_path: Path):
        f = tmp_path / "test.json"
        f.write_text("content")
        await delete_file(f)
        assert not f.exists()

    @pytest.mark.asyncio
    async def test_ignores_missing(self, tmp_path: Path):
        await delete_file(tmp_path / "nonexistent.json")  # Should not raise
