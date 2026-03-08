"""IPC file operations: atomic writes, dispatch scanning, result/nudge writing."""

import asyncio
import contextlib
import json
import os
import secrets
import time
from pathlib import Path

from worker_manager.schemas import Dispatch, Nudge, Result


def generate_filename() -> str:
    """Generate a filename matching NanoClaw convention: {timestamp_ms}-{random6}.json."""
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
    Returns list of (path, dispatch) tuples sorted by filename (chronological).
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
    """Write a result file to the results directory. Returns the file path."""
    filename = generate_filename()
    path = results_dir / filename
    await atomic_write(path, result.model_dump_json(indent=2))
    return path


async def write_nudge(input_dir: Path, nudge: Nudge) -> Path:
    """Write a nudge file wrapped in NanoClaw's IPC message envelope.

    NanoClaw's drainIpcInput() only processes {"type": "message", "text": "..."} files.
    The Nudge JSON is serialized into the text field of this envelope.
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
