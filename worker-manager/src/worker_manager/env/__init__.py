"""Environment preflight validation.

Re-exports the main orchestrator function and EnvReport.
"""

import logging
from pathlib import Path

from worker_manager.env.loader import (
    discover_env_vars_from_dotenv_example,
    load_env_layers,
    parse_tanren_yml,
)
from worker_manager.env.reporter import format_report
from worker_manager.env.schema import EnvBlock, OnMissing
from worker_manager.env.validator import EnvReport, validate_env

__all__ = ["EnvReport", "format_report", "load_and_validate_env"]

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
    """
    import asyncio

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

    merged_env, source_map = await asyncio.to_thread(load_env_layers, project_root, secrets_dir)

    report = validate_env(env_block, merged_env, source_map)

    return report, merged_env
