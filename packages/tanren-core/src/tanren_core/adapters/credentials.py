"""Credential injection for remote CLI execution.

Each CLI's auth needs are encapsulated in a CredentialProvider.
All credentials are injected at provision time; execute does no auth injection.
"""

from __future__ import annotations

import json
import logging
from pathlib import Path
from typing import TYPE_CHECKING, Protocol, runtime_checkable

from tanren_core.adapters.remote_types import SecretBundle
from tanren_core.schemas import Cli

if TYPE_CHECKING:
    from tanren_core.adapters.protocols import RemoteConnection

logger = logging.getLogger(__name__)


async def _resolve_remote_home(conn: RemoteConnection) -> str:
    """Resolve the remote user's home directory.

    Returns:
        Absolute path to the remote home directory.

    Raises:
        RuntimeError: If the ``echo $HOME`` command fails.
    """
    result = await conn.run("echo $HOME", timeout=10)
    home = result.stdout.strip()
    if not home or result.exit_code != 0:
        raise RuntimeError(f"Failed to resolve remote $HOME: {result.stderr}")
    return home


@runtime_checkable
class CredentialProvider(Protocol):
    """Protocol for CLI credential providers."""

    @property
    def name(self) -> str:
        """Human-readable provider name."""
        ...

    @property
    def cleanup_paths(self) -> tuple[str, ...]:
        """Shell paths (tilde-prefixed) to remove during cleanup."""
        ...

    async def inject(
        self, conn: RemoteConnection, secrets: SecretBundle, *, home_dir: str | None = None
    ) -> bool:
        """Inject credentials onto the remote host.

        Args:
            conn: RemoteConnection to the VM.
            secrets: SecretBundle with credential data.
            home_dir: If set, use this as the remote home instead of $HOME.

        Returns:
            True if credentials were written, False if the key was missing.
        """
        ...


class OpencodeCredentialProvider:
    """Inject opencode auth.json for Z.ai API key auth."""

    _SUFFIX = ".local/share/opencode/auth.json"
    _SHELL_PATH = f"~/{_SUFFIX}"

    @property
    def name(self) -> str:
        """Return provider name."""
        return "opencode"

    @property
    def cleanup_paths(self) -> tuple[str, ...]:
        """Return shell paths to clean up."""
        return (self._SHELL_PATH,)

    async def inject(
        self, conn: RemoteConnection, secrets: SecretBundle, *, home_dir: str | None = None
    ) -> bool:
        """Write opencode auth.json if OPENCODE_ZAI_API_KEY is present.

        Returns:
            True if credentials were written, False if the key was missing.
        """
        zai_key = secrets.developer.get("OPENCODE_ZAI_API_KEY") or secrets.project.get(
            "OPENCODE_ZAI_API_KEY"
        )
        if not zai_key:
            logger.warning("OPENCODE_ZAI_API_KEY not found in secrets — opencode auth skipped")
            return False

        auth_data = {"zai-coding-plan": {"type": "api", "key": zai_key}}
        content = json.dumps(auth_data, indent=2)

        remote_home = home_dir or await _resolve_remote_home(conn)
        auth_dir = f"{remote_home}/{Path(self._SUFFIX).parent}"
        abs_auth_path = f"{remote_home}/{self._SUFFIX}"
        await conn.run(f"mkdir -p {auth_dir}", timeout=10)
        await conn.upload_content(content, abs_auth_path)
        await conn.run(f"chmod 600 {abs_auth_path}", timeout=10)
        if home_dir:
            user = Path(home_dir).name
            await conn.run(f"chown -R {user}:{user} {auth_dir}", timeout=10)
        logger.info("Injected opencode auth.json (zai-coding-plan)")
        return True


class ClaudeCredentialProvider:
    """Inject Claude credentials.json for subscription (Max/Pro) auth."""

    _SUFFIX = ".claude/.credentials.json"
    _SHELL_PATH = f"~/{_SUFFIX}"

    @property
    def name(self) -> str:
        """Return provider name."""
        return "claude"

    @property
    def cleanup_paths(self) -> tuple[str, ...]:
        """Return shell paths to clean up."""
        return (self._SHELL_PATH,)

    async def inject(
        self, conn: RemoteConnection, secrets: SecretBundle, *, home_dir: str | None = None
    ) -> bool:
        """Write Claude credentials.json if CLAUDE_CREDENTIALS_JSON is present.

        Returns:
            True if credentials were written, False if the key was missing.
        """
        content = secrets.developer.get("CLAUDE_CREDENTIALS_JSON")
        if not content:
            logger.warning("CLAUDE_CREDENTIALS_JSON not found in secrets — claude auth skipped")
            return False

        remote_home = home_dir or await _resolve_remote_home(conn)
        auth_dir = f"{remote_home}/{Path(self._SUFFIX).parent}"
        abs_auth_path = f"{remote_home}/{self._SUFFIX}"
        await conn.run(f"mkdir -p {auth_dir}", timeout=10)
        await conn.upload_content(content, abs_auth_path)
        await conn.run(f"chmod 600 {abs_auth_path}", timeout=10)
        if home_dir:
            user = Path(home_dir).name
            await conn.run(f"chown -R {user}:{user} {auth_dir}", timeout=10)
        logger.info("Injected claude credentials.json (subscription)")
        return True


class CodexCredentialProvider:
    """Inject Codex auth.json for subscription (ChatGPT Plus) auth."""

    _SUFFIX = ".codex/auth.json"
    _SHELL_PATH = f"~/{_SUFFIX}"

    @property
    def name(self) -> str:
        """Return provider name."""
        return "codex"

    @property
    def cleanup_paths(self) -> tuple[str, ...]:
        """Return shell paths to clean up."""
        return (self._SHELL_PATH,)

    async def inject(
        self, conn: RemoteConnection, secrets: SecretBundle, *, home_dir: str | None = None
    ) -> bool:
        """Write Codex auth.json if CODEX_AUTH_JSON is present.

        Returns:
            True if credentials were written, False if the key was missing.
        """
        content = secrets.developer.get("CODEX_AUTH_JSON")
        if not content:
            logger.warning("CODEX_AUTH_JSON not found in secrets — codex auth skipped")
            return False

        remote_home = home_dir or await _resolve_remote_home(conn)
        auth_dir = f"{remote_home}/{Path(self._SUFFIX).parent}"
        abs_auth_path = f"{remote_home}/{self._SUFFIX}"
        await conn.run(f"mkdir -p {auth_dir}", timeout=10)
        await conn.upload_content(content, abs_auth_path)
        await conn.run(f"chmod 600 {abs_auth_path}", timeout=10)
        if home_dir:
            user = Path(home_dir).name
            await conn.run(f"chown -R {user}:{user} {auth_dir}", timeout=10)
        logger.info("Injected codex auth.json (subscription)")
        return True


DEFAULT_CREDENTIAL_PROVIDERS: tuple[CredentialProvider, ...] = (
    OpencodeCredentialProvider(),
    ClaudeCredentialProvider(),
    CodexCredentialProvider(),
)

CLI_CREDENTIAL_PROVIDERS: dict[Cli, type[CredentialProvider]] = {
    Cli.OPENCODE: OpencodeCredentialProvider,
    Cli.CLAUDE: ClaudeCredentialProvider,
    Cli.CODEX: CodexCredentialProvider,
}


def providers_for_clis(clis: frozenset[Cli]) -> tuple[CredentialProvider, ...]:
    """Build credential provider instances for the given CLI set.

    Returns:
        Tuple of CredentialProvider instances, sorted by CLI value.
    """
    return tuple(
        CLI_CREDENTIAL_PROVIDERS[cli]()
        for cli in sorted(clis, key=lambda c: c.value)
        if cli in CLI_CREDENTIAL_PROVIDERS
    )


async def inject_all_cli_credentials(
    conn: RemoteConnection,
    secrets: SecretBundle,
    providers: tuple[CredentialProvider, ...] = DEFAULT_CREDENTIAL_PROVIDERS,
    *,
    target_home: str | None = None,
) -> list[str]:
    """Inject all CLI credentials onto the remote host.

    Args:
        conn: RemoteConnection to the VM.
        secrets: SecretBundle with credential data.
        providers: Credential providers to inject.
        target_home: If set, inject credentials into this home directory.

    Returns:
        List of provider names that were successfully injected.
    """
    injected: list[str] = []
    for provider in providers:
        try:
            if await provider.inject(conn, secrets, home_dir=target_home):
                injected.append(provider.name)
        except Exception:
            logger.warning("Credential injection failed for %s", provider.name, exc_info=True)
    return injected


def all_credential_cleanup_paths(
    providers: tuple[CredentialProvider, ...] = DEFAULT_CREDENTIAL_PROVIDERS,
) -> list[str]:
    """Aggregate cleanup paths from all providers.

    Returns:
        List of shell paths to remove during cleanup.
    """
    paths: list[str] = []
    for provider in providers:
        paths.extend(provider.cleanup_paths)
    return paths
