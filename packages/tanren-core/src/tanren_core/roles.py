"""Agent role definitions and mapping."""

from enum import StrEnum

from pydantic import BaseModel, ConfigDict, Field

from tanren_core.schemas import AuthMode, Cli

# Backward-compatible alias.
CliAuthMethod = AuthMode


class RoleName(StrEnum):
    """Supported workflow role names."""

    DEFAULT = "default"
    CONVERSATION = "conversation"
    IMPLEMENTATION = "implementation"
    AUDIT = "audit"
    FEEDBACK = "feedback"
    CONFLICT_RESOLUTION = "conflict_resolution"


class AgentTool(BaseModel):
    """Configuration for a specific agent tool."""

    model_config = ConfigDict(extra="forbid")

    cli: Cli = Field(...)
    model: str | None = Field(default=None)
    endpoint: str | None = Field(default=None)
    auth: AuthMode = Field(default=AuthMode.API_KEY)
    cli_path: str | None = Field(default=None)


class RoleMapping(BaseModel):
    """Maps workflow roles to agent tools.

    Each role can be fulfilled by a different CLI + model combination.
    Missing roles fall back to the default.
    """

    model_config = ConfigDict(extra="forbid")

    default: AgentTool = Field(...)
    conversation: AgentTool | None = Field(default=None)
    implementation: AgentTool | None = Field(default=None)
    audit: AgentTool | None = Field(default=None)
    feedback: AgentTool | None = Field(default=None)
    conflict_resolution: AgentTool | None = Field(default=None)

    def resolve(self, role: RoleName | str) -> AgentTool:
        """Resolve a role to its agent tool, falling back to default.

        Returns:
            AgentTool for the given role.
        """
        role_name = role.value if isinstance(role, RoleName) else role
        resolved = getattr(self, role_name, None)
        return resolved or self.default

    def required_clis(self) -> frozenset[Cli]:
        """Return the set of CLIs needed by all configured roles.

        Bash is excluded because it does not require installation or
        credential injection.
        """
        clis: set[Cli] = {self.default.cli}
        for field in ("conversation", "implementation", "audit", "feedback", "conflict_resolution"):
            tool = getattr(self, field)
            if tool is not None:
                clis.add(tool.cli)
        clis.discard(Cli.BASH)
        return frozenset(clis)
