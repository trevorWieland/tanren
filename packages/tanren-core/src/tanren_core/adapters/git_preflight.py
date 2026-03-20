"""Git-backed preflight runner adapter."""

from __future__ import annotations

from typing import TYPE_CHECKING

from tanren_core.preflight import PreflightResult, run_preflight

if TYPE_CHECKING:
    from pathlib import Path


class GitPreflightRunner:
    """Delegates to preflight.run_preflight()."""

    async def run(
        self, worktree_path: Path, branch: str, spec_folder: Path, phase: str
    ) -> PreflightResult:
        """Run pre-flight checks before spawning an agent process.

        Returns:
            PreflightResult with check outcomes and file snapshots.
        """
        return await run_preflight(worktree_path, branch, spec_folder, phase)
