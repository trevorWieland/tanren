"""Git-backed postflight runner adapter."""

from __future__ import annotations

from pathlib import Path

from worker_manager.postflight import PostflightResult, run_postflight


class GitPostflightRunner:
    """Delegates to postflight.run_postflight()."""

    async def run(
        self,
        worktree_path: Path,
        branch: str,
        phase: str,
        preflight_hashes: dict[str, str],
        preflight_backups: dict[str, str],
        *,
        skip_push: bool = False,
    ) -> PostflightResult:
        return await run_postflight(
            worktree_path, branch, phase, preflight_hashes, preflight_backups,
            skip_push=skip_push,
        )
