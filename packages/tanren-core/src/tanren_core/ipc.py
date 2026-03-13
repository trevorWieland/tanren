"""IPC file operations: atomic writes, dispatch scanning, result/nudge writing."""

import asyncio
import contextlib
import json
import os
import re
import secrets
import time
from datetime import UTC, datetime
from pathlib import Path

from tanren_core.schemas import Dispatch, Nudge, ProgressState, Result, TaskState


def generate_filename() -> str:
    """Generate a filename matching IPC convention: {timestamp_ms}-{random6}.json.

    Returns:
        Filename string like ``1234567890-abc123.json``.
    """
    timestamp = int(time.time() * 1000)
    random_hex = secrets.token_hex(3)
    return f"{timestamp}-{random_hex}.json"


async def atomic_write(path: Path, content: str) -> None:
    """Write content atomically: write .tmp, fsync, rename."""

    def _write() -> None:
        tmp_path = path.with_suffix(".tmp")
        with open(tmp_path, "w") as f:
            f.write(content)
            f.flush()
            os.fsync(f.fileno())
        tmp_path.rename(path)

    await asyncio.to_thread(_write)


async def scan_dispatch_dir(dispatch_dir: Path) -> list[tuple[Path, Dispatch]]:
    """Scan dispatch directory for .json files, parse and sort by filename.

    Ignores .tmp files per PROTOCOL.md atomic write protocol.

    Returns:
        List of (path, dispatch) tuples sorted by filename (chronological).
    """

    def _scan() -> list[tuple[Path, Dispatch]]:
        if not dispatch_dir.exists():
            return []
        results: list[tuple[Path, Dispatch]] = []
        for entry in sorted(dispatch_dir.iterdir()):
            if entry.suffix != ".json":
                continue
            try:
                content = entry.read_text()
                dispatch = Dispatch.model_validate_json(content)
                results.append((entry, dispatch))
            except Exception:
                # Skip unparseable files — don't crash the poll loop
                continue
        return results

    return await asyncio.to_thread(_scan)


async def write_result(results_dir: Path, result: Result) -> Path:
    """Write a result file to the results directory.

    Returns:
        Path to the written result file.
    """
    filename = generate_filename()
    path = results_dir / filename
    await atomic_write(path, result.model_dump_json(indent=2))
    return path


async def write_nudge(input_dir: Path, nudge: Nudge) -> Path:
    """Write a nudge file wrapped in the coordinator's IPC message envelope.

    The coordinator's IPC input handler only processes {"type": "message", "text": "..."} files.
    The Nudge JSON is serialized into the text field of this coordinator IPC message envelope.

    Returns:
        Path to the written nudge file.
    """
    filename = generate_filename()
    path = input_dir / filename
    envelope = json.dumps({
        "type": "message",
        "text": nudge.model_dump_json(),
    })
    await atomic_write(path, envelope)
    return path


async def delete_file(path: Path) -> None:
    """Delete a file, ignoring FileNotFoundError."""

    def _delete() -> None:
        with contextlib.suppress(FileNotFoundError):
            path.unlink()

    await asyncio.to_thread(_delete)


async def read_progress(path: Path) -> ProgressState:
    """Read and parse progress.json.

    Returns:
        Parsed ProgressState.
    """

    def _read() -> ProgressState:
        content = path.read_text()
        return ProgressState.model_validate_json(content)

    return await asyncio.to_thread(_read)


async def write_progress(path: Path, state: ProgressState) -> None:
    """Write progress.json atomically, updating updated_at."""
    state.updated_at = datetime.now(UTC).isoformat()
    await atomic_write(path, state.model_dump_json(indent=2))


async def init_progress_from_plan(plan_md_path: Path, spec_id: str) -> ProgressState:
    """Parse plan.md for '- [ ] Task N: {title}' lines, create ProgressState.

    Returns:
        ProgressState initialized from plan.md tasks.
    """

    def _parse() -> ProgressState:
        content = plan_md_path.read_text()
        tasks = []
        for match in re.finditer(r"^\s*- \[[ x]\] Task (\d+):\s*(.+)$", content, re.MULTILINE):
            tasks.append(TaskState(id=int(match.group(1)), title=match.group(2).strip()))
        now = datetime.now(UTC).isoformat()
        return ProgressState(spec_id=spec_id, created_at=now, updated_at=now, tasks=tasks)

    return await asyncio.to_thread(_parse)
