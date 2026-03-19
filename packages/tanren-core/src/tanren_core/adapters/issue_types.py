"""Data types for issue tracker adapters."""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict, Field


class Issue(BaseModel):
    """Full issue detail from a tracker."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    id: str = Field(..., description="Issue identifier (e.g. '144' or 'PROJ-123')")
    title: str = Field(..., description="Issue title")
    body: str = Field(default="", description="Issue body / description")
    status: str = Field(default="", description="Current status label")
    labels: tuple[str, ...] = Field(default_factory=tuple, description="Attached labels")
    url: str = Field(default="", description="Web URL for the issue")
    metadata: dict[str, str] = Field(default_factory=dict, description="Provider-specific metadata")


class IssueSummary(BaseModel):
    """Lightweight issue summary for listings."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    id: str = Field(..., description="Issue identifier")
    title: str = Field(..., description="Issue title")
    status: str = Field(default="", description="Current status label")
    url: str = Field(default="", description="Web URL for the issue")
