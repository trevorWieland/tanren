"""Environment validation orchestrator — parse config, load layers, validate."""

import asyncio
import logging
from typing import TYPE_CHECKING

from tanren_core.env.loader import (
    discover_env_vars_from_dotenv_example,
    load_env_layers,
    parse_tanren_yml,
)
from tanren_core.env.schema import EnvBlock, OnMissing
from tanren_core.env.secret_provider_factory import create_secret_provider
from tanren_core.env.validator import EnvReport, validate_env

if TYPE_CHECKING:
    from pathlib import Path

logger = logging.getLogger(__name__)


async def load_and_validate_env(
    project_root: Path,
    daemon_mode: bool = True,
    secrets_dir: Path | None = None,
) -> tuple[EnvReport, dict[str, str]]:
    """Orchestrator: parse config, load layers, validate, return report + env dict.

    In daemon_mode, on_missing is forced to 'error' (ignores 'prompt' policy).
    If no tanren.yml env block exists, falls back to .env.example.
    If neither exists, returns a passing report (no requirements).

    Returns:
        Tuple of (EnvReport, merged env dict).
    """
    config = await asyncio.to_thread(parse_tanren_yml, project_root)

    env_block: EnvBlock | None = None

    if config and config.env:
        env_block = config.env
    else:
        # Fallback to .env.example
        required_vars = await asyncio.to_thread(discover_env_vars_from_dotenv_example, project_root)
        if required_vars:
            env_block = EnvBlock(required=required_vars)

    if env_block is None:
        # No env requirements — pass
        return EnvReport(passed=True), {}

    # In daemon mode, force error policy (no interactive prompts)
    if daemon_mode and env_block.on_missing == OnMissing.PROMPT:
        env_block = env_block.model_copy(update={"on_missing": OnMissing.ERROR})

    # Only build a secret provider if at least one var declares a source
    secret_provider = None
    has_sources = any(v.source for v in (*env_block.required, *env_block.optional))
    if has_sources:
        secrets_config = config.secrets if config else None
        secret_provider = create_secret_provider(secrets_config, secrets_dir=secrets_dir)

    merged_env, source_map = await asyncio.to_thread(load_env_layers, project_root, secrets_dir)

    report = await validate_env(env_block, merged_env, source_map, secret_provider=secret_provider)

    return report, merged_env
