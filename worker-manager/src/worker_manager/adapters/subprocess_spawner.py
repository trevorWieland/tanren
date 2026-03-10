"""Subprocess-based process spawner adapter."""

from __future__ import annotations

from pathlib import Path

from worker_manager.config import Config
from worker_manager.process import ProcessResult, spawn_process
from worker_manager.schemas import Dispatch


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
        return await spawn_process(dispatch, worktree_path, config, task_env=task_env)
