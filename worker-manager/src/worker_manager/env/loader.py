"""Layered .env loading and tanren.yml parsing.

Priority (highest wins):
1. os.environ (real environment)
2. Project-local .env file (via python-dotenv)
3. secrets.d/*.env files (alphabetical)
4. secrets.env
5. tanren.yml optional defaults (lowest)

Does NOT mutate os.environ — returns a dict.
"""

import logging
import os
import stat
from pathlib import Path

import yaml
from dotenv import dotenv_values

from worker_manager.env.schema import RequiredEnvVar, TanrenConfig
from worker_manager.env.secrets import DEFAULT_SECRETS_DIR

logger = logging.getLogger(__name__)


def parse_tanren_yml(project_root: Path) -> TanrenConfig | None:
    """Parse tanren.yml from project root into a TanrenConfig model.

    Returns None if the file does not exist.
    """
    yml_path = project_root / "tanren.yml"
    if not yml_path.exists():
        return None

    raw = yaml.safe_load(yml_path.read_text())
    if not raw:
        return None

    return TanrenConfig.model_validate(raw)


def discover_env_vars_from_dotenv_example(project_root: Path) -> list[RequiredEnvVar]:
    """Fallback: parse .env.example key names (no pattern/hint support).

    Logs a deprecation warning encouraging migration to tanren.yml env block.
    """
    example_path = project_root / ".env.example"
    if not example_path.exists():
        return []

    logger.warning(
        "Using .env.example for env requirements is deprecated — "
        "add an 'env' block to tanren.yml instead (run 'tanren env init')"
    )

    values = dotenv_values(example_path)
    return [RequiredEnvVar(key=k) for k in values]


def _check_permissions(path: Path) -> None:
    """Warn if a secrets file is world-readable."""
    try:
        mode = path.stat().st_mode
        if mode & stat.S_IROTH:
            logger.warning(
                "Secrets file %s is world-readable (mode %o) — run 'chmod 600 %s' to fix",
                path,
                stat.S_IMODE(mode),
                path,
            )
    except OSError:
        pass


def load_env_layers(
    project_root: Path,
    secrets_dir: Path | None = None,
) -> tuple[dict[str, str], dict[str, str]]:
    """Load env vars from multiple sources with priority.

    Returns (merged_env, source_map) where source_map tracks where each key
    came from (e.g. "os.environ", ".env", "secrets.env").
    """
    sd = secrets_dir or DEFAULT_SECRETS_DIR

    merged: dict[str, str] = {}
    source_map: dict[str, str] = {}

    # Layer 5 (lowest): tanren.yml optional defaults — handled by validator

    # Layer 4: secrets.env
    secrets_path = sd / "secrets.env"
    if secrets_path.exists():
        _check_permissions(secrets_path)
        values = dotenv_values(secrets_path)
        for k, v in values.items():
            if v is not None:
                merged[k] = v
                source_map[k] = str(secrets_path)

    # Layer 3: secrets.d/*.env (alphabetical)
    secrets_d = sd / "secrets.d"
    if secrets_d.is_dir():
        for env_file in sorted(secrets_d.glob("*.env")):
            _check_permissions(env_file)
            values = dotenv_values(env_file)
            for k, v in values.items():
                if v is not None:
                    merged[k] = v
                    source_map[k] = str(env_file)

    # Layer 2: project-local .env
    dotenv_path = project_root / ".env"
    if dotenv_path.exists():
        values = dotenv_values(dotenv_path)
        for k, v in values.items():
            if v is not None:
                merged[k] = v
                source_map[k] = ".env"

    # Layer 1 (highest): real environment
    for k in merged:
        if k in os.environ:
            merged[k] = os.environ[k]
            source_map[k] = "os.environ"

    # Also pick up any env vars that are in os.environ but not yet in merged
    # (these will be checked against required/optional lists by the validator)
    # We don't add ALL of os.environ — only vars already tracked or needed by schema.
    # The validator will query os.environ for required/optional keys not in merged.

    return merged, source_map


def resolve_env_var(
    key: str,
    merged: dict[str, str],
    source_map: dict[str, str],
) -> tuple[str | None, str | None]:
    """Resolve a single env var from merged dict or os.environ.

    Returns (value, source).
    """
    if key in merged:
        return merged[key], source_map.get(key)

    val = os.environ.get(key)
    if val is not None:
        return val, "os.environ"

    return None, None
