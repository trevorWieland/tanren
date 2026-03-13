"""Generate .env in worktrees from resolved env layers.

Reads tanren.yml from the worktree to determine required/optional vars,
resolves values from all env sources (main repo .env, secrets store, os.environ),
and writes a .env file to the worktree.
"""

import logging
from pathlib import Path

from tanren_core.env.loader import load_env_layers, parse_tanren_yml, resolve_env_var

logger = logging.getLogger(__name__)


def provision_worktree_env(
    worktree_path: Path,
    project_dir: Path,
    secrets_dir: Path | None = None,
) -> int:
    """Generate .env in worktree from resolved env layers.

    Reads tanren.yml from the worktree to determine required/optional vars,
    resolves values from all env sources (main repo .env, secrets store, os.environ),
    and writes a .env file to the worktree.

    Returns:
        Number of env vars written to the worktree .env file.
    """
    config = parse_tanren_yml(worktree_path)
    if config is None or config.env is None:
        return 0

    env_block = config.env
    keys = [v.key for v in env_block.required] + [v.key for v in env_block.optional]
    if not keys:
        return 0

    merged, source_map = load_env_layers(project_dir, secrets_dir)

    lines: list[str] = []
    for key in keys:
        value, _source = resolve_env_var(key, merged, source_map)
        if value is not None:
            lines.append(f"{key}={value}")

    if not lines:
        return 0

    dotenv_path = worktree_path / ".env"
    dotenv_path.write_text("\n".join(lines) + "\n")
    return len(lines)
