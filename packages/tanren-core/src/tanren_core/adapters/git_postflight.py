"""Git-backed postflight runner adapter."""

from __future__ import annotations

from typing import TYPE_CHECKING

from tanren_core.postflight import PostflightResult, run_postflight

if TYPE_CHECKING:
    from pathlib import Path


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
        """Run post-flight integrity checks and push committed changes.

        Returns:
            PostflightResult with repair and push status.
        """
        return await run_postflight(
            worktree_path,
            branch,
            phase,
            preflight_hashes,
            preflight_backups,
            skip_push=skip_push,
        )
