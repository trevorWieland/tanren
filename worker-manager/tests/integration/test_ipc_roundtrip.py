"""Integration test: IPC roundtrip with real filesystem."""

import json
from pathlib import Path

import pytest

from worker_manager.ipc import (
    atomic_write,
    delete_file,
    scan_dispatch_dir,
    write_nudge,
    write_result,
)
from worker_manager.schemas import (
    Dispatch,
    Nudge,
    Outcome,
    Phase,
    Result,
)


class TestIPCRoundtrip:
    @pytest.mark.asyncio
    async def test_dispatch_scan_and_delete(self, tmp_path: Path):
        """Write dispatches, scan them, delete them."""
        dispatch_dir = tmp_path / "dispatch"
        dispatch_dir.mkdir()

        # Write two dispatch files
        for i, wf_id in enumerate(["wf-rentl-1-1000", "wf-rentl-2-2000"]):
            d = Dispatch(
                workflow_id=wf_id,
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
            path = dispatch_dir / f"100{i}-aaaaaa.json"
            await atomic_write(path, d.model_dump_json())

        # Scan
        results = await scan_dispatch_dir(dispatch_dir)
        assert len(results) == 2
        assert results[0][1].workflow_id == "wf-rentl-1-1000"
        assert results[1][1].workflow_id == "wf-rentl-2-2000"

        # Delete
        for path, _ in results:
            await delete_file(path)

        # Verify empty
        results = await scan_dispatch_dir(dispatch_dir)
        assert results == []

    @pytest.mark.asyncio
    async def test_result_write_and_read(self, tmp_path: Path):
        """Write a result, read it back, verify schema."""
        result = Result(
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

        path = await write_result(tmp_path, result)

        # Read back and verify
        data = json.loads(path.read_text())
        parsed = Result.model_validate(data)
        assert parsed == result

    @pytest.mark.asyncio
    async def test_nudge_write_and_read(self, tmp_path: Path):
        """Write a nudge, read it back, verify NanoClaw message envelope."""
        nudge = Nudge(workflow_id="wf-rentl-144-1741359600")
        path = await write_nudge(tmp_path, nudge)

        envelope = json.loads(path.read_text())
        assert envelope["type"] == "message"
        assert "text" in envelope
        inner = json.loads(envelope["text"])
        assert inner["type"] == "workflow_result"
        assert inner["workflow_id"] == "wf-rentl-144-1741359600"

    @pytest.mark.asyncio
    async def test_concurrent_writes(self, tmp_path: Path):
        """Multiple concurrent atomic writes should not corrupt files."""
        import asyncio

        async def write_one(i: int) -> Path:
            r = Result(
                workflow_id=f"wf-test-{i}-1000",
                phase=Phase.GATE,
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
            return await write_result(tmp_path, r)

        paths = await asyncio.gather(*[write_one(i) for i in range(10)])
        assert len(paths) == 10
        assert len(set(paths)) == 10  # All unique

        for p in paths:
            data = json.loads(p.read_text())
            Result.model_validate(data)  # All parseable
