"""Dotenv-based environment provisioner adapter."""

from __future__ import annotations

from typing import TYPE_CHECKING

from tanren_core.env.provision import provision_worktree_env

if TYPE_CHECKING:
    from pathlib import Path


class DotenvEnvProvisioner:
    """Delegates to env.provision.provision_worktree_env().

    Sync method — caller wraps in asyncio.to_thread().
    """

    def provision(self, worktree_path: Path, project_dir: Path) -> int:
        """Provision .env files from the project directory into the worktree.

        Returns:
            Number of .env files provisioned.
        """
        return provision_worktree_env(worktree_path, project_dir)
