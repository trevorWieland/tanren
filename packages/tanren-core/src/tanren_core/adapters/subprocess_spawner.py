"""Subprocess-based process spawner adapter."""

from __future__ import annotations

from typing import TYPE_CHECKING

from tanren_core.process import ProcessResult, spawn_process

if TYPE_CHECKING:
    from pathlib import Path

    from tanren_core.schemas import Dispatch
    from tanren_core.worker_config import WorkerConfig


class SubprocessSpawner:
    """Delegates to process.spawn_process()."""

    async def spawn(
        self,
        dispatch: Dispatch,
        worktree_path: Path,
        config: WorkerConfig,
        *,
        task_env: dict[str, str] | None = None,
    ) -> ProcessResult:
        """Spawn a subprocess for the given dispatch and return the result.

        Returns:
            ProcessResult with exit code, stdout, and timeout info.
        """
        return await spawn_process(dispatch, worktree_path, config, task_env=task_env)
