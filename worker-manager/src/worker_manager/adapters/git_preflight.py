"""Git-backed preflight runner adapter."""

from __future__ import annotations

from pathlib import Path

from worker_manager.preflight import PreflightResult, run_preflight


class GitPreflightRunner:
    """Delegates to preflight.run_preflight()."""

    async def run(
        self, worktree_path: Path, branch: str, spec_folder: Path, phase: str
    ) -> PreflightResult:
        return await run_preflight(worktree_path, branch, spec_folder, phase)
