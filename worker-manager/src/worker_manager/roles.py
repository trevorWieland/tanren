"""Agent role definitions and mapping."""

from dataclasses import dataclass


@dataclass
class AgentTool:
    """Configuration for a specific agent tool."""

    cli: str
    model: str | None = None
    endpoint: str | None = None
    auth: str = "api_key"
    cli_path: str | None = None


@dataclass
class RoleMapping:
    """Maps workflow roles to agent tools.

    Each role can be fulfilled by a different CLI + model combination.
    Missing roles fall back to the default.
    """

    default: AgentTool
    conversation: AgentTool | None = None
    implementation: AgentTool | None = None
    audit: AgentTool | None = None
    feedback: AgentTool | None = None
    conflict_resolution: AgentTool | None = None

    def resolve(self, role: str) -> AgentTool:
        """Resolve a role to its agent tool, falling back to default."""
        return getattr(self, role, None) or self.default
