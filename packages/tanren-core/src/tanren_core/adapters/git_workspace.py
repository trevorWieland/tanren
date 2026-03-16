"""Remote git workspace management."""

from __future__ import annotations

import json
import logging
import shlex
from pathlib import Path
from typing import TYPE_CHECKING

from pydantic import BaseModel, ConfigDict, Field

from tanren_core.adapters.remote_types import SecretBundle, WorkspacePath, WorkspaceSpec
from tanren_core.remote_config import GitAuthMethod
from tanren_core.roles import AuthMode
from tanren_core.schemas import Cli

if TYPE_CHECKING:
    from tanren_core.adapters.protocols import RemoteConnection

logger = logging.getLogger(__name__)


def _shell_quote(value: str) -> str:
    """Quote a value for safe sourcing in bash.

    Returns:
        Shell-quoted string.
    """
    return "'" + value.replace("'", "'\\''") + "'"


class GitAuthConfig(BaseModel):
    """Git authentication configuration."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    auth_method: GitAuthMethod = Field(default=GitAuthMethod.TOKEN)
    token: str | None = Field(default=None)
    ssh_key_path: str | None = Field(default=None)


class GitWorkspaceManager:
    """Manages git workspaces on remote VMs.

    Implements the WorkspaceManager protocol. Handles clone/pull,
    branch checkout, secret injection, and cleanup.
    """

    def __init__(self, auth: GitAuthConfig) -> None:
        """Initialize with git authentication configuration."""
        self._auth = auth
        self._injected_opencode_auth = False

    _ASKPASS_PATH = "/workspace/.git-askpass"

    async def _setup_git_auth(self, conn: RemoteConnection) -> None:
        """Upload a GIT_ASKPASS helper so the token never appears in process args."""
        if self._auth.auth_method == GitAuthMethod.TOKEN and self._auth.token:
            script = f"#!/bin/sh\necho {_shell_quote(self._auth.token)}\n"
            await conn.upload_content(script, self._ASKPASS_PATH)
            await conn.run(f"chmod 700 {self._ASKPASS_PATH}", timeout=10)

    def _git_env_prefix(self) -> str:
        """Return env prefix for git commands when using token auth."""
        if self._auth.auth_method == GitAuthMethod.TOKEN and self._auth.token:
            return f"GIT_ASKPASS={self._ASKPASS_PATH} GIT_TERMINAL_PROMPT=0 "
        return ""

    async def setup(self, conn: RemoteConnection, spec: WorkspaceSpec) -> WorkspacePath:
        """Clone or pull repo, checkout branch, run setup commands.

        Returns:
            WorkspacePath with the cloned workspace details.

        Raises:
            RuntimeError: If git clone, pull, or a setup command fails.
        """
        workspace_dir = f"/workspace/{spec.project}"

        # Setup askpass-based auth
        await self._setup_git_auth(conn)
        git_prefix = self._git_env_prefix()

        quoted_dir = shlex.quote(workspace_dir)

        # Check if already cloned
        check = await conn.run(f"test -d {quoted_dir}/.git && echo exists", timeout=10)

        if "exists" in check.stdout:
            # Pull latest
            logger.info("Pulling latest for %s on branch %s", spec.project, spec.branch)
            pull_cmd = (
                f"cd {quoted_dir} && {git_prefix}git fetch origin"
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
                f" {shlex.quote(spec.repo_url)} {quoted_dir}"
            )
            result = await conn.run(clone_cmd, timeout=300)
            if result.exit_code != 0:
                raise RuntimeError(f"Git clone failed: {result.stderr}")

        # Run setup commands (PATH includes bootstrap-installed tools)
        path_prefix = 'export PATH="$HOME/.opencode/bin:$HOME/.local/bin:$PATH" && '
        for cmd in spec.setup_commands:
            logger.info("Running setup command: %s", cmd)
            result = await conn.run(f"{path_prefix}cd {quoted_dir} && {cmd}", timeout=300)
            if result.exit_code != 0:
                raise RuntimeError(f"Setup command failed ({cmd}): {result.stderr}")

        return WorkspacePath(
            path=workspace_dir,
            project=spec.project,
            branch=spec.branch,
        )

    _OPENCODE_AUTH_PATH = "/root/.local/share/opencode/auth.json"

    async def inject_secrets(
        self,
        conn: RemoteConnection,
        workspace: WorkspacePath,
        secrets: SecretBundle,
        *,
        cli_auth: tuple[Cli, AuthMode] | None = None,
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
            await conn.run(f"chmod 600 {shlex.quote(env_path)}", timeout=10)

        # CLI-specific auth files
        if cli_auth is not None:
            await self.inject_cli_auth(conn, secrets, cli_auth)

    async def inject_cli_auth(
        self,
        conn: RemoteConnection,
        secrets: SecretBundle,
        cli_auth: tuple[Cli, AuthMode],
    ) -> None:
        """Set up CLI-specific auth files based on auth mode."""
        cli, auth = cli_auth

        # Clear stale opencode auth if switching away from opencode/api_key
        if self._injected_opencode_auth and not (cli == Cli.OPENCODE and auth == AuthMode.API_KEY):
            await conn.run(f"rm -f {self._OPENCODE_AUTH_PATH}", timeout=10)
            self._injected_opencode_auth = False

        if cli == Cli.OPENCODE and auth == AuthMode.API_KEY:
            await self._inject_opencode_api_key(conn, secrets)

    async def _inject_opencode_api_key(
        self,
        conn: RemoteConnection,
        secrets: SecretBundle,
    ) -> None:
        """Write opencode auth.json for Z.ai API key auth."""
        zai_key = secrets.developer.get("OPENCODE_ZAI_API_KEY") or secrets.project.get(
            "OPENCODE_ZAI_API_KEY"
        )
        if not zai_key:
            logger.warning("OPENCODE_ZAI_API_KEY not found in secrets — opencode auth will fail")
            return

        auth_data = {"zai-coding-plan": {"type": "api", "key": zai_key}}
        content = json.dumps(auth_data, indent=2)

        auth_dir = str(Path(self._OPENCODE_AUTH_PATH).parent)
        await conn.run(f"mkdir -p {shlex.quote(auth_dir)}", timeout=10)
        await conn.upload_content(content, self._OPENCODE_AUTH_PATH)
        await conn.run(f"chmod 600 {self._OPENCODE_AUTH_PATH}", timeout=10)
        logger.info("Injected opencode auth.json (zai-coding-plan)")
        self._injected_opencode_auth = True

    def push_command(self, workspace_path: str, branch: str) -> str:
        """Return an auth-prefixed git push command string."""
        quoted_path = shlex.quote(workspace_path)
        quoted_branch = shlex.quote(branch)
        return f"cd {quoted_path} && {self._git_env_prefix()}git push origin {quoted_branch}"

    async def cleanup(self, conn: RemoteConnection, workspace: WorkspacePath) -> None:
        """Remove secret files and auth helpers from remote workspace."""
        await conn.run("rm -f /workspace/.developer-secrets", timeout=10)
        await conn.run(f"rm -f {shlex.quote(workspace.path + '/.env')}", timeout=10)
        await conn.run(f"rm -f {self._ASKPASS_PATH}", timeout=10)
        if self._injected_opencode_auth:
            await conn.run(f"rm -f {self._OPENCODE_AUTH_PATH}", timeout=10)
