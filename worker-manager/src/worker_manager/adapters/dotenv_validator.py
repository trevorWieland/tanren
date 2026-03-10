"""Dotenv-based environment validator adapter."""

from __future__ import annotations

from pathlib import Path

from worker_manager.env import load_and_validate_env
from worker_manager.env.validator import EnvReport


class DotenvEnvValidator:
    """Delegates to env.load_and_validate_env()."""

    async def load_and_validate(self, project_root: Path) -> tuple[EnvReport, dict[str, str]]:
        return await load_and_validate_env(project_root)
