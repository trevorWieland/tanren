"""Dotenv-based environment validator adapter."""

from __future__ import annotations

from pathlib import Path

from tanren_core.env import load_and_validate_env
from tanren_core.env.validator import EnvReport


class DotenvEnvValidator:
    """Delegates to env.load_and_validate_env()."""

    async def load_and_validate(self, project_root: Path) -> tuple[EnvReport, dict[str, str]]:
        """Load and validate environment variables for the given project root.

        Returns:
            Tuple of (validation report, resolved env vars).
        """
        return await load_and_validate_env(project_root)
