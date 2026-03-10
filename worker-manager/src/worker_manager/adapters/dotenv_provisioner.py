"""Dotenv-based environment provisioner adapter."""

from __future__ import annotations

from pathlib import Path

from worker_manager.env.provision import provision_worktree_env


class DotenvEnvProvisioner:
    """Delegates to env.provision.provision_worktree_env().

    Sync method — caller wraps in asyncio.to_thread().
    """

    def provision(self, worktree_path: Path, project_dir: Path) -> int:
        return provision_worktree_env(worktree_path, project_dir)
