"""Load role mapping configuration from YAML."""

from __future__ import annotations

import logging
from collections.abc import Mapping
from pathlib import Path
from typing import TYPE_CHECKING

import yaml

from tanren_core.roles import AgentTool, AuthMode, RoleMapping
from tanren_core.schemas import Cli

if TYPE_CHECKING:
    from pydantic import JsonValue

logger = logging.getLogger(__name__)

_VALID_CLIS = {c.value for c in Cli}


def load_roles_config(path: str | Path) -> RoleMapping:
    """Load role mapping from a YAML file.

    Returns:
        RoleMapping parsed from YAML.

    Raises:
        FileNotFoundError: If the config file does not exist.
        TypeError: If the YAML structure is not a mapping or missing required sections.
    """
    path = Path(path)
    if not path.exists():
        raise FileNotFoundError(f"Roles config not found: {path}")

    data = yaml.safe_load(path.read_text())
    if not isinstance(data, Mapping):
        raise TypeError(f"Roles config {path}: expected a mapping, got {type(data).__name__}")

    agents = data.get("agents")
    if not isinstance(agents, Mapping):
        raise TypeError(f"Roles config {path}: missing required 'agents' section")

    def _as_str(raw: JsonValue) -> str | None:
        return raw if isinstance(raw, str) else None

    def _parse_cli(source: Mapping[str, JsonValue], context: str) -> Cli:
        if "cli" not in source:
            raise ValueError(f"Roles config {path}: {context} missing required 'cli' field")
        cli_raw = _as_str(source["cli"])
        if cli_raw is None or cli_raw not in _VALID_CLIS:
            allowed = sorted(_VALID_CLIS)
            raise ValueError(f"Invalid CLI value '{cli_raw}'. Must be one of: {allowed}")
        return Cli(cli_raw)

    def _parse_auth(source: Mapping[str, JsonValue], context: str) -> AuthMode:
        if "auth" not in source:
            raise ValueError(f"Roles config {path}: {context} missing required 'auth' field")
        auth_raw = _as_str(source["auth"])
        if auth_raw is None:
            raise ValueError(f"Roles config {path}: {context} 'auth' must be a string")
        return AuthMode(auth_raw)

    def _parse_model(source: Mapping[str, JsonValue], context: str) -> str:
        if "model" not in source:
            raise ValueError(f"Roles config {path}: {context} missing required 'model' field")
        model_raw = _as_str(source["model"])
        if model_raw is None:
            raise ValueError(f"Roles config {path}: {context} 'model' must be a string")
        return model_raw

    def _parse_tool(tool_data: Mapping[str, JsonValue], context: str) -> AgentTool:
        return AgentTool(
            cli=_parse_cli(tool_data, context),
            model=_parse_model(tool_data, context),
            endpoint=_as_str(tool_data.get("endpoint")),
            auth=_parse_auth(tool_data, context),
            cli_path=_as_str(tool_data.get("cli_path")),
        )

    default_source = agents.get("default")
    if not isinstance(default_source, Mapping):
        raise TypeError(
            f"Roles config {path}: agents.default must be a mapping with at least 'cli'"
        )
    default_tool = _parse_tool(default_source, "agents.default")

    role_tools: dict[str, AgentTool] = {"default": default_tool}
    for role in ("conversation", "implementation", "audit", "feedback", "conflict_resolution"):
        yaml_key = role.replace("_", "-")
        role_source = agents.get(yaml_key)
        if not isinstance(role_source, Mapping):
            role_source = agents.get(role)
        if isinstance(role_source, Mapping):
            role_tools[role] = _parse_tool(role_source, f"agents.{yaml_key}")

    return RoleMapping.model_validate(role_tools)
