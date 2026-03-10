"""Remote git workspace management."""

from __future__ import annotations

import logging
import shlex
from dataclasses import dataclass

from worker_manager.adapters.remote_types import SecretBundle, WorkspacePath, WorkspaceSpec

logger = logging.getLogger(__name__)


def _shell_quote(value: str) -> str:
    """Quote a value for safe sourcing in bash."""
    return "'" + value.replace("'", "'\\''") + "'"


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

    _ASKPASS_PATH = "/workspace/.git-askpass"

    async def _setup_git_auth(self, conn) -> None:
        """Upload a GIT_ASKPASS helper so the token never appears in process args."""
        if (
            self._auth.auth_method == "token"
            and self._auth.token
        ):
            script = (
                "#!/bin/sh\n"
                f"echo {_shell_quote(self._auth.token)}\n"
            )
            await conn.upload_content(script, self._ASKPASS_PATH)
            await conn.run(f"chmod 700 {self._ASKPASS_PATH}", timeout=10)

    def _git_env_prefix(self) -> str:
        """Return env prefix for git commands when using token auth."""
        if self._auth.auth_method == "token" and self._auth.token:
            return f"GIT_ASKPASS={self._ASKPASS_PATH} GIT_TERMINAL_PROMPT=0 "
        return ""

    async def setup(self, conn, spec: WorkspaceSpec) -> WorkspacePath:
        """Clone or pull repo, checkout branch, run setup commands."""
        workspace_dir = f"/workspace/{spec.project}"

        # Setup askpass-based auth
        await self._setup_git_auth(conn)
        git_prefix = self._git_env_prefix()

        # Check if already cloned
        check = await conn.run(f"test -d {workspace_dir}/.git && echo exists", timeout=10)

        if "exists" in check.stdout:
            # Pull latest
            logger.info("Pulling latest for %s on branch %s", spec.project, spec.branch)
            pull_cmd = (
                f"cd {workspace_dir} && {git_prefix}git fetch origin"
                f" && git checkout {shlex.quote(spec.branch)}"
                f" && {git_prefix}git pull origin {shlex.quote(spec.branch)}"
            )
            result = await conn.run(pull_cmd, timeout=120)
            if result.exit_code != 0:
                raise RuntimeError(f"Git pull failed: {result.stderr}")
        else:
            # Clone fresh
            logger.info("Cloning %s branch %s", spec.project, spec.branch)
            clone_cmd = (
                f"{git_prefix}git clone --branch {shlex.quote(spec.branch)}"
                f" {shlex.quote(spec.repo_url)} {workspace_dir}"
            )
            result = await conn.run(clone_cmd, timeout=300)
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
            lines = [f"{k}={_shell_quote(v)}" for k, v in secrets.developer.items()]
            content = "\n".join(lines) + "\n"
            await conn.upload_content(content, "/workspace/.developer-secrets")
            await conn.run("chmod 600 /workspace/.developer-secrets", timeout=10)

        # Project secrets -> /workspace/{project}/.env
        if secrets.project:
            lines = [f"{k}={_shell_quote(v)}" for k, v in secrets.project.items()]
            content = "\n".join(lines) + "\n"
            env_path = f"{workspace.path}/.env"
            await conn.upload_content(content, env_path)
            await conn.run(f"chmod 600 {env_path}", timeout=10)

    async def cleanup(self, conn, workspace: WorkspacePath) -> None:
        """Remove secret files and auth helpers from remote workspace."""
        await conn.run("rm -f /workspace/.developer-secrets", timeout=10)
        await conn.run(f"rm -f {workspace.path}/.env", timeout=10)
        await conn.run(f"rm -f {self._ASKPASS_PATH}", timeout=10)
