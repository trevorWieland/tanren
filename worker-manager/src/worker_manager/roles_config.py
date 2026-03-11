"""Load role mapping configuration from YAML."""

import logging
from collections.abc import Mapping
from pathlib import Path

import yaml

from worker_manager.roles import AgentTool, AuthMode, RoleMapping
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
        return RoleMapping(default=AgentTool(cli=Cli.CLAUDE))

    data = yaml.safe_load(path.read_text())
    if not isinstance(data, Mapping):
        return RoleMapping(default=AgentTool(cli=Cli.CLAUDE))

    agents = data.get("agents")
    if not isinstance(agents, Mapping):
        return RoleMapping(default=AgentTool(cli=Cli.CLAUDE))

    def _as_str(raw: object) -> str | None:
        return raw if isinstance(raw, str) else None

    def _parse_cli(raw: object) -> Cli:
        cli_raw = _as_str(raw) or Cli.CLAUDE.value
        if cli_raw not in _VALID_CLIS:
            allowed = sorted(_VALID_CLIS)
            raise ValueError(f"Invalid CLI value '{cli_raw}'. Must be one of: {allowed}")
        return Cli(cli_raw)

    def _parse_auth(raw: object) -> AuthMode:
        auth_raw = _as_str(raw) or AuthMode.API_KEY.value
        return AuthMode(auth_raw)

    def _parse_tool(tool_data: Mapping[str, object] | None) -> AgentTool:
        source = tool_data if tool_data is not None else {}
        return AgentTool(
            cli=_parse_cli(source.get("cli")),
            model=_as_str(source.get("model")),
            endpoint=_as_str(source.get("endpoint")),
            auth=_parse_auth(source.get("auth")),
            cli_path=_as_str(source.get("cli_path")),
        )

    default_source = agents.get("default")
    default_tool = (
        _parse_tool(default_source)
        if isinstance(default_source, Mapping)
        else AgentTool(cli=Cli.CLAUDE)
    )

    role_tools: dict[str, AgentTool] = {"default": default_tool}
    for role in ("conversation", "implementation", "audit", "feedback", "conflict_resolution"):
        yaml_key = role.replace("_", "-")
        role_source = agents.get(yaml_key)
        if not isinstance(role_source, Mapping):
            role_source = agents.get(role)
        if isinstance(role_source, Mapping):
            role_tools[role] = _parse_tool(role_source)

    return RoleMapping.model_validate(role_tools)
