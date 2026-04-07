"""Config resolver — abstract config file access for profile resolution.

Provides a ``ConfigResolver`` protocol that abstracts how project config files
(tanren.yml, .env) are read.  Two implementations:

- ``DiskConfigResolver`` — reads from a local git checkout (CLI, daemon)
- ``GitHubConfigResolver`` — fetches via GitHub raw content API (API, MCP)

Infrastructure config (remote.yml, roles.yml) is NOT part of this protocol;
it stays on the daemon/API host filesystem and is loaded via ``WorkerConfig``.
"""

from __future__ import annotations

import logging
import re
from pathlib import Path
from typing import TYPE_CHECKING, Protocol, runtime_checkable

if TYPE_CHECKING:
    from collections.abc import Callable

import yaml
from dotenv import dotenv_values

logger = logging.getLogger(__name__)


@runtime_checkable
class ConfigResolver(Protocol):
    """Protocol for loading project config files."""

    async def load_tanren_config(self, project: str, branch: str = "main") -> dict:
        """Load parsed tanren.yml content for a project.

        Returns:
            Parsed YAML as a dict, or empty dict if not found.
        """
        ...

    async def load_project_env(self, project: str) -> dict[str, str]:
        """Load .env key-value pairs for a project.

        Returns:
            Dict of env var key-value pairs, or empty dict if not found.
        """
        ...


class DiskConfigResolver:
    """Read project config from a local git checkout directory."""

    def __init__(self, github_dir: str) -> None:
        """Initialize with the root directory containing project repos."""
        self._github_dir = Path(github_dir)

    async def load_tanren_config(self, project: str, branch: str = "main") -> dict:
        """Load tanren.yml from disk. ``branch`` is ignored (uses current checkout).

        Returns:
            Parsed YAML as a dict, or empty dict if not found.
        """
        _ = branch
        tanren_yml = self._github_dir / project / "tanren.yml"
        if not tanren_yml.exists():
            return {}
        loaded = yaml.safe_load(tanren_yml.read_text()) or {}
        return loaded if isinstance(loaded, dict) else {}

    async def load_project_env(self, project: str) -> dict[str, str]:
        """Load .env from disk.

        Returns:
            Dict of env var key-value pairs, or empty dict if not found.
        """
        env_file = self._github_dir / project / ".env"
        if not env_file.exists():
            return {}
        values = dotenv_values(env_file)
        return {k: v for k, v in values.items() if v is not None}


# Regex to parse GitHub repo URLs (supports dotted repo names like my.repo.git)
_GITHUB_URL_RE = re.compile(
    r"(?:https?://github\.com/|git@github\.com:)"
    r"(?P<owner>[^/]+)/(?P<repo>[^/]+?)(?:\.git)?$"
)


class GitHubConfigResolver:
    """Fetch project config via GitHub raw content API.

    Requires a mapping from project name to repo URL (typically from
    ``RemoteConfig.repo_url_for``) and an optional GitHub token for
    private repos.
    """

    def __init__(
        self,
        repo_url_for: Callable[[str], str | None],
        token: str | None = None,
    ) -> None:
        """Initialize with repo URL lookup and optional auth token."""
        self._repo_url_for = repo_url_for
        self._token = token

    def _parse_owner_repo(self, repo_url: str) -> tuple[str, str] | None:
        """Extract (owner, repo) from a GitHub URL.

        Returns:
            Tuple of (owner, repo) or None if URL doesn't match.
        """
        m = _GITHUB_URL_RE.search(repo_url)
        if m:
            return m.group("owner"), m.group("repo")
        return None

    async def _fetch_raw(self, owner: str, repo: str, branch: str, path: str) -> str | None:
        """Fetch a file from GitHub raw content API.

        Returns:
            File content as string, or None if not found.
        """
        import httpx  # noqa: PLC0415

        url = f"https://raw.githubusercontent.com/{owner}/{repo}/{branch}/{path}"
        headers: dict[str, str] = {}
        if self._token:
            headers["Authorization"] = f"token {self._token}"

        async with httpx.AsyncClient(timeout=15) as client:
            resp = await client.get(url, headers=headers)
            if resp.status_code == 404:
                return None
            resp.raise_for_status()
            return resp.text

    async def load_tanren_config(self, project: str, branch: str = "main") -> dict:
        """Fetch tanren.yml from GitHub.

        Returns:
            Parsed YAML as a dict, or empty dict if not found.
        """
        repo_url = self._repo_url_for(project)
        if not repo_url:
            logger.debug("No repo URL for project %s — returning empty config", project)
            return {}

        parsed = self._parse_owner_repo(repo_url)
        if parsed is None:
            logger.warning("Cannot parse GitHub URL: %s", repo_url)
            return {}

        owner, repo = parsed
        content = await self._fetch_raw(owner, repo, branch, "tanren.yml")
        if content is None:
            return {}

        loaded = yaml.safe_load(content) or {}
        return loaded if isinstance(loaded, dict) else {}

    async def load_project_env(self, project: str) -> dict[str, str]:
        """Fetch .env from GitHub.

        Note: .env files are typically gitignored. This will return empty
        for most projects. Callers should provide ``project_env`` overrides
        when using GitHubConfigResolver.

        Returns:
            Dict of env var key-value pairs, or empty dict if not found.
        """
        repo_url = self._repo_url_for(project)
        if not repo_url:
            return {}

        parsed = self._parse_owner_repo(repo_url)
        if parsed is None:
            return {}

        owner, repo = parsed
        content = await self._fetch_raw(owner, repo, "main", ".env")
        if content is None:
            return {}

        # Parse dotenv-format content from string
        result: dict[str, str] = {}
        for line in content.splitlines():
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            if "=" in line:
                key, _, value = line.partition("=")
                key = key.strip()
                value = value.strip().strip("'\"")
                if key:
                    result[key] = value
        return result
