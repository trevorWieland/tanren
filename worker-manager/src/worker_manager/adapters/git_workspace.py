"""Remote git workspace management."""

from __future__ import annotations

import logging
from dataclasses import dataclass

from worker_manager.adapters.remote_types import SecretBundle, WorkspacePath, WorkspaceSpec

logger = logging.getLogger(__name__)


@dataclass(frozen=True)
class GitAuthConfig:
    """Git authentication configuration."""

    auth_method: str = "token"  # "token" or "ssh"
    token: str | None = None
    ssh_key_path: str | None = None


class GitWorkspaceManager:
    """Manages git workspaces on remote VMs.

    Implements the WorkspaceManager protocol. Handles clone/pull,
    branch checkout, secret injection, and cleanup.
    """

    def __init__(self, auth: GitAuthConfig) -> None:
        self._auth = auth

    def _build_repo_url(self, repo_url: str) -> str:
        """Build authenticated repo URL. Token is used inline, never written to disk."""
        if (
            self._auth.auth_method == "token"
            and self._auth.token
            and repo_url.startswith("https://")
        ):
            return repo_url.replace("https://", f"https://{self._auth.token}@", 1)
        return repo_url

    async def setup(self, conn, spec: WorkspaceSpec) -> WorkspacePath:
        """Clone or pull repo, checkout branch, run setup commands."""
        workspace_dir = f"/workspace/{spec.project}"
        auth_url = self._build_repo_url(spec.repo_url)

        # Check if already cloned
        check = await conn.run(f"test -d {workspace_dir}/.git && echo exists", timeout=10)

        if "exists" in check.stdout:
            # Pull latest
            logger.info("Pulling latest for %s on branch %s", spec.project, spec.branch)
            pull_cmd = (
                f"cd {workspace_dir} && git fetch origin"
                f" && git checkout {spec.branch}"
                f" && git pull origin {spec.branch}"
            )
            result = await conn.run(pull_cmd, timeout=120)
            if result.exit_code != 0:
                raise RuntimeError(f"Git pull failed: {result.stderr}")
        else:
            # Clone fresh
            logger.info("Cloning %s branch %s", spec.project, spec.branch)
            result = await conn.run(
                f"git clone --branch {spec.branch} {auth_url} {workspace_dir}",
                timeout=300,
            )
            if result.exit_code != 0:
                raise RuntimeError(f"Git clone failed: {result.stderr}")

        # Run setup commands
        for cmd in spec.setup_commands:
            logger.info("Running setup command: %s", cmd)
            result = await conn.run(f"cd {workspace_dir} && {cmd}", timeout=300)
            if result.exit_code != 0:
                raise RuntimeError(f"Setup command failed ({cmd}): {result.stderr}")

        return WorkspacePath(
            path=workspace_dir,
            project=spec.project,
            branch=spec.branch,
        )

    async def inject_secrets(
        self, conn, workspace: WorkspacePath, secrets: SecretBundle
    ) -> None:
        """Write secret files to remote workspace. Files are chmod 600."""
        # Developer secrets -> /workspace/.developer-secrets
        if secrets.developer:
            lines = [f"{k}={v}" for k, v in secrets.developer.items()]
            content = "\n".join(lines) + "\n"
            await conn.upload_content(content, "/workspace/.developer-secrets")
            await conn.run("chmod 600 /workspace/.developer-secrets", timeout=10)

        # Project secrets -> /workspace/{project}/.env
        if secrets.project:
            lines = [f"{k}={v}" for k, v in secrets.project.items()]
            content = "\n".join(lines) + "\n"
            env_path = f"{workspace.path}/.env"
            await conn.upload_content(content, env_path)
            await conn.run(f"chmod 600 {env_path}", timeout=10)

    async def cleanup(self, conn, workspace: WorkspacePath) -> None:
        """Remove secret files from remote workspace."""
        await conn.run("rm -f /workspace/.developer-secrets", timeout=10)
        await conn.run(f"rm -f {workspace.path}/.env", timeout=10)
