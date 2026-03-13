"""Subprocess-based process spawner adapter."""

from __future__ import annotations

from pathlib import Path

from tanren_core.config import Config
from tanren_core.process import ProcessResult, spawn_process
from tanren_core.schemas import Dispatch


class SubprocessSpawner:
    """Delegates to process.spawn_process()."""

    async def spawn(
        self,
        dispatch: Dispatch,
        worktree_path: Path,
        config: Config,
        *,
        task_env: dict[str, str] | None = None,
    ) -> ProcessResult:
        """Spawn a subprocess for the given dispatch and return the result.

        Returns:
            ProcessResult with exit code, stdout, and timeout info.
        """
        return await spawn_process(dispatch, worktree_path, config, task_env=task_env)
