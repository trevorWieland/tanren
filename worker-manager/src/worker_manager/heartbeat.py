"""Heartbeat writer for crash detection per PROTOCOL.md Section 10."""

import asyncio
import contextlib
import logging
import time
from pathlib import Path

logger = logging.getLogger(__name__)


class HeartbeatWriter:
    """Manages heartbeat files in the in-progress directory.

    Creates a heartbeat file per dispatch, updates it every interval seconds,
    and deletes it when the dispatch completes.
    """

    def __init__(self, in_progress_dir: Path, interval: float = 30.0) -> None:
        self._dir = in_progress_dir
        self._interval = interval
        self._tasks: dict[str, asyncio.Task[None]] = {}

    def _heartbeat_path(self, dispatch_stem: str) -> Path:
        return self._dir / f"{dispatch_stem}.heartbeat"

    async def start(self, dispatch_stem: str) -> None:
        """Create heartbeat file and start background update task."""
        path = self._heartbeat_path(dispatch_stem)
        await self._write_heartbeat(path)
        task = asyncio.create_task(
            self._update_loop(path, dispatch_stem),
            name=f"heartbeat-{dispatch_stem}",
        )
        self._tasks[dispatch_stem] = task

    async def stop(self, dispatch_stem: str) -> None:
        """Cancel update task and delete heartbeat file."""
        task = self._tasks.pop(dispatch_stem, None)
        if task:
            task.cancel()
            with contextlib.suppress(asyncio.CancelledError):
                await task

        path = self._heartbeat_path(dispatch_stem)
        with contextlib.suppress(FileNotFoundError):
            path.unlink()

    async def cleanup_stale(self) -> None:
        """On startup, find heartbeats > 60s old and delete them."""
        if not self._dir.exists():
            return

        now = time.time()
        for entry in self._dir.iterdir():
            if entry.suffix != ".heartbeat":
                continue
            try:
                content = entry.read_text().strip()
                ts = float(content)
                if now - ts > 60:
                    logger.info("Cleaning up stale heartbeat: %s", entry.name)
                    entry.unlink()
            except (ValueError, OSError):
                # Can't parse or read — delete it
                with contextlib.suppress(FileNotFoundError):
                    entry.unlink()

    async def _update_loop(self, path: Path, dispatch_stem: str) -> None:
        """Background task: update heartbeat file every interval seconds."""
        try:
            while True:
                await asyncio.sleep(self._interval)
                await self._write_heartbeat(path)
        except asyncio.CancelledError:
            pass

    @staticmethod
    async def _write_heartbeat(path: Path) -> None:
        """Write current timestamp to heartbeat file."""

        def _write() -> None:
            path.write_text(str(time.time()))

        await asyncio.to_thread(_write)
