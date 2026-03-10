"""Load role mapping configuration from YAML."""

import logging
from pathlib import Path

import yaml

from worker_manager.roles import AgentTool, RoleMapping
from worker_manager.schemas import Cli

logger = logging.getLogger(__name__)

_VALID_CLIS = {c.value for c in Cli}


def load_roles_config(path: str | Path) -> RoleMapping:
    """Load role mapping from a YAML file.

    Returns a default RoleMapping (all claude-code) if the file is missing.
    Raises ValueError if a CLI value is not in the Cli enum.
    """
    path = Path(path)
    if not path.exists():
        logger.info("Roles config not found at %s, using defaults", path)
        return RoleMapping(default=AgentTool(cli="claude"))

    data = yaml.safe_load(path.read_text())
    if not data or "agents" not in data:
        return RoleMapping(default=AgentTool(cli="claude"))

    agents = data["agents"]

    def _parse_tool(tool_data: dict) -> AgentTool:
        cli = tool_data.get("cli", "claude")
        if cli not in _VALID_CLIS:
            raise ValueError(f"Invalid CLI value '{cli}'. Must be one of: {sorted(_VALID_CLIS)}")
        return AgentTool(
            cli=cli,
            model=tool_data.get("model"),
            endpoint=tool_data.get("endpoint"),
            auth=tool_data.get("auth", "api_key"),
            cli_path=tool_data.get("cli_path"),
        )

    default = _parse_tool(agents.get("default", {"cli": "claude"}))

    kwargs: dict = {"default": default}
    for role in ("conversation", "implementation", "audit", "feedback", "conflict_resolution"):
        yaml_key = role.replace("_", "-")
        if yaml_key in agents:
            kwargs[role] = _parse_tool(agents[yaml_key])
        elif role in agents:
            kwargs[role] = _parse_tool(agents[role])

    return RoleMapping(**kwargs)
