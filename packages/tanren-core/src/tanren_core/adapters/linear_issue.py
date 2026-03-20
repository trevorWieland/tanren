"""Linear issue source adapter using GraphQL API."""

from __future__ import annotations

import asyncio
import logging
import os
from typing import TYPE_CHECKING, cast

from pydantic import BaseModel, ConfigDict, Field, JsonValue

from tanren_core.adapters.issue_types import Issue, IssueSummary

if TYPE_CHECKING:
    import types
    from collections.abc import Mapping

    import httpx

logger = logging.getLogger(__name__)

_LINEAR_GRAPHQL_URL = "https://api.linear.app/graphql"


def _import_httpx() -> types.ModuleType:
    """Import httpx at runtime.

    Returns:
        The httpx module.

    Raises:
        ImportError: If httpx is not installed.
    """
    try:
        import httpx as _httpx  # noqa: PLC0415 — deferred import for optional dependency

        return _httpx
    except ImportError:
        raise ImportError(
            "httpx is required for the Linear issue adapter. "
            "Install it with: uv sync --extra linear"
        ) from None


class LinearIssueSettings(BaseModel):
    """Configuration for Linear issue source."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    api_key_env: str = Field(
        default="LINEAR_API_KEY",
        description="Environment variable name containing the Linear API key",
    )
    team_key: str = Field(
        ...,
        description="Linear team key prefix (e.g. 'PROJ' for PROJ-123 identifiers)",
    )
    api_url: str = Field(
        default=_LINEAR_GRAPHQL_URL,
        description="Linear GraphQL API endpoint",
    )
    status_mapping: dict[str, str] = Field(
        default_factory=lambda: {
            "draft": "Backlog",
            "shaped": "Todo",
            "executing": "In Progress",
            "validating": "In Review",
            "review": "In Review",
            "merged": "Done",
        },
        description="Map of tanren lifecycle states to Linear workflow state names",
    )

    @classmethod
    def from_settings(cls, settings: Mapping[str, JsonValue]) -> LinearIssueSettings:
        """Parse from tanren.yml issue_source.settings.

        Returns:
            Validated LinearIssueSettings.
        """
        return cls.model_validate(settings)


_GET_ISSUE_QUERY = """
query($id: String!) {
  issue(id: $id) {
    id
    identifier
    title
    description
    state { id name type }
    labels(first: 20) { nodes { name } }
    url
    team { id key }
  }
}
"""

_LIST_ISSUES_QUERY = """
query($teamKey: String, $stateName: String) {
  issues(filter: {
    team: { key: { eq: $teamKey } }
    state: { name: { eq: $stateName } }
  }) {
    nodes {
      identifier
      title
      state { name }
      url
    }
  }
}
"""

_LIST_ISSUES_NO_STATUS_QUERY = """
query($teamKey: String) {
  issues(filter: {
    team: { key: { eq: $teamKey } }
  }) {
    nodes {
      identifier
      title
      state { name }
      url
    }
  }
}
"""

_GET_TEAM_STATES_QUERY = """
query($teamId: String!) {
  team(id: $teamId) {
    states {
      nodes {
        id
        name
        type
      }
    }
  }
}
"""

_UPDATE_ISSUE_MUTATION = """
mutation($id: String!, $stateId: String!) {
  issueUpdate(id: $id, input: { stateId: $stateId }) {
    success
    issue { state { name } }
  }
}
"""

_CREATE_COMMENT_MUTATION = """
mutation($issueId: String!, $body: String!) {
  commentCreate(input: { issueId: $issueId, body: $body }) {
    success
  }
}
"""

type _JsonDict = dict[str, object]


def _as_dict(
    val: object,
) -> _JsonDict:  # Receives parsed JSON of unknown shape; object is intentional
    """Cast a JSON value known to be a dict to ``dict[str, object]``.

    Call only after confirming ``isinstance(val, dict)``.

    Returns:
        The value cast to ``dict[str, object]``.
    """
    return cast("_JsonDict", val)


class LinearIssueSource:
    """Fetch and update Linear issues via GraphQL API."""

    def __init__(self, settings: LinearIssueSettings) -> None:
        """Initialize with Linear issue settings.

        Args:
            settings: Validated Linear issue settings.

        Raises:
            ValueError: If the API key environment variable is not set.
        """
        api_key = os.environ.get(settings.api_key_env)
        if not api_key:
            raise ValueError(
                f"Missing Linear API key in environment variable: {settings.api_key_env}"
            )
        httpx_mod = _import_httpx()
        self._client: httpx.Client = httpx_mod.Client(
            headers={"Authorization": api_key, "Content-Type": "application/json"},
            timeout=30.0,
        )
        self._settings = settings
        self._api_url = settings.api_url
        self._state_cache: dict[str, str] = {}
        self._team_id: str | None = None

    def _execute(self, query: str, variables: dict[str, object]) -> dict[str, object]:
        """Execute a GraphQL query/mutation.

        Args:
            query: GraphQL query or mutation string.
            variables: Variables for the query.

        Returns:
            The ``data`` dict from the GraphQL response.

        Raises:
            RuntimeError: If the response contains GraphQL errors.
        """
        payload: dict[str, object] = {"query": query, "variables": variables}
        response = self._client.post(self._api_url, json=payload)
        response.raise_for_status()
        body = response.json()
        if not isinstance(body, dict):
            raise RuntimeError("Unexpected GraphQL response format")
        if "errors" in body:
            errors = body["errors"]
            raise RuntimeError(f"GraphQL errors: {errors}")
        data = body.get("data")
        if not isinstance(data, dict):
            raise RuntimeError("Missing 'data' in GraphQL response")
        return data

    @staticmethod
    def _build_issue(data: dict[str, object]) -> Issue:
        """Map a GraphQL issue node to an Issue model.

        Args:
            data: The issue node from a GraphQL response.

        Returns:
            An Issue model instance.
        """
        linear_id = str(data.get("id", ""))
        identifier = str(data.get("identifier", ""))
        title = str(data.get("title", ""))
        description = str(data.get("description") or "")
        url = str(data.get("url", ""))

        state_data = data.get("state")
        status = ""
        state_type = ""
        if isinstance(state_data, dict):
            s = _as_dict(state_data)
            status = str(s.get("name", ""))
            state_type = str(s.get("type", ""))

        labels_data = data.get("labels")
        label_names: list[str] = []
        if isinstance(labels_data, dict):
            nodes = _as_dict(labels_data).get("nodes")
            if isinstance(nodes, list):
                for node in nodes:
                    if isinstance(node, dict):
                        name = _as_dict(node).get("name")
                        if isinstance(name, str):
                            label_names.append(name)

        team_data = data.get("team")
        team_key = ""
        team_id = ""
        if isinstance(team_data, dict):
            t = _as_dict(team_data)
            team_key = str(t.get("key", ""))
            team_id = str(t.get("id", ""))

        metadata: dict[str, str] = {
            "linear_id": linear_id,
            "team_key": team_key,
            "team_id": team_id,
            "state_type": state_type,
        }

        return Issue(
            id=identifier,
            title=title,
            body=description,
            status=status,
            labels=tuple(label_names),
            url=url,
            metadata=metadata,
        )

    @staticmethod
    def _build_summary(data: dict[str, object]) -> IssueSummary:
        """Map a GraphQL issue node to an IssueSummary model.

        Args:
            data: The issue node from a GraphQL response.

        Returns:
            An IssueSummary model instance.
        """
        identifier = str(data.get("identifier", ""))
        title = str(data.get("title", ""))
        url = str(data.get("url", ""))
        state_data = data.get("state")
        status = ""
        if isinstance(state_data, dict):
            status = str(_as_dict(state_data).get("name", ""))
        return IssueSummary(id=identifier, title=title, status=status, url=url)

    async def get_issue(self, issue_id: str) -> Issue:
        """Fetch full issue detail by ID.

        Args:
            issue_id: Linear issue identifier (e.g. ``"PROJ-123"``).

        Returns:
            The Issue model for the requested issue.

        Raises:
            ValueError: If the issue is not found.
        """
        variables: dict[str, object] = {"id": issue_id}
        data = await asyncio.to_thread(self._execute, _GET_ISSUE_QUERY, variables)
        issue_data = data.get("issue")
        if not isinstance(issue_data, dict):
            raise ValueError(f"Issue {issue_id} not found")
        issue = self._build_issue(_as_dict(issue_data))
        # Cache or update team ID based on the fetched issue.
        team_id = issue.metadata.get("team_id", "")
        if team_id and team_id != self._team_id:
            self._team_id = team_id
            self._state_cache.clear()
        return issue

    async def list_issues(
        self, *, project: str | None = None, status: str | None = None
    ) -> list[IssueSummary]:
        """List issues, optionally filtered by status.

        Args:
            project: Override team key filter. Defaults to ``settings.team_key``.
            status: Filter by Linear workflow state name. None returns all.

        Returns:
            List of IssueSummary instances.
        """
        team_key = project if project is not None else self._settings.team_key
        if status is not None:
            variables: dict[str, object] = {"teamKey": team_key, "stateName": status}
            data = await asyncio.to_thread(self._execute, _LIST_ISSUES_QUERY, variables)
        else:
            variables = {"teamKey": team_key}
            data = await asyncio.to_thread(self._execute, _LIST_ISSUES_NO_STATUS_QUERY, variables)
        issues_data = data.get("issues")
        if not isinstance(issues_data, dict):
            return []
        nodes = _as_dict(issues_data).get("nodes")
        if not isinstance(nodes, list):
            return []
        summaries: list[IssueSummary] = [
            self._build_summary(_as_dict(node)) for node in nodes if isinstance(node, dict)
        ]
        return summaries

    async def update_status(self, issue_id: str, status: str) -> None:
        """Transition an issue to a new status.

        Maps tanren lifecycle status to a Linear workflow state name via
        ``settings.status_mapping``, then resolves the state name to a UUID
        for the ``issueUpdate`` mutation.

        Args:
            issue_id: Linear issue identifier (e.g. ``"PROJ-123"``).
            status: Tanren lifecycle status string.
        """
        normalized = status.strip().lower()
        target_name = self._settings.status_mapping.get(normalized)
        if target_name is None:
            logger.warning(
                "No Linear status mapping for tanren status %r; skipping update for %s",
                status,
                issue_id,
            )
            return

        # Ensure team context is available for state resolution.
        await self.get_issue(issue_id)
        state_id = await asyncio.to_thread(self._resolve_state_id, target_name)
        if state_id is None:
            logger.warning(
                "Linear workflow state %r not found for team; skipping update for %s",
                target_name,
                issue_id,
            )
            return

        variables: dict[str, object] = {"id": issue_id, "stateId": state_id}
        await asyncio.to_thread(self._execute, _UPDATE_ISSUE_MUTATION, variables)

    async def add_comment(self, issue_id: str, body: str) -> None:
        """Append a comment to an issue.

        Resolves the internal UUID via ``get_issue`` because Linear's
        ``commentCreate`` mutation requires the UUID, not the identifier.

        Args:
            issue_id: Linear issue identifier (e.g. ``"PROJ-123"``).
            body: Comment body text.
        """
        issue = await self.get_issue(issue_id)
        linear_id = issue.metadata.get("linear_id", "")
        variables: dict[str, object] = {"issueId": linear_id, "body": body}
        await asyncio.to_thread(self._execute, _CREATE_COMMENT_MUTATION, variables)

    def _resolve_state_id(self, state_name: str) -> str | None:
        """Resolve a workflow state name to its UUID.

        Fetches and caches team workflow states on first call.

        Args:
            state_name: Linear workflow state name (e.g. ``"In Progress"``).

        Returns:
            The state UUID, or None if not found.
        """
        if not self._state_cache:
            if not self._team_id:
                logger.warning(
                    "Cannot resolve state %r: team ID not yet known (call get_issue first)",
                    state_name,
                )
                return None
            variables: dict[str, object] = {"teamId": self._team_id}
            data = self._execute(_GET_TEAM_STATES_QUERY, variables)
            team_data = data.get("team")
            if isinstance(team_data, dict):
                states_data = _as_dict(team_data).get("states")
                if isinstance(states_data, dict):
                    nodes = _as_dict(states_data).get("nodes")
                    if isinstance(nodes, list):
                        for node in nodes:
                            if isinstance(node, dict):
                                n = _as_dict(node)
                                name = n.get("name")
                                sid = n.get("id")
                                if isinstance(name, str) and isinstance(sid, str):
                                    self._state_cache[name.lower()] = sid
        return self._state_cache.get(state_name.lower())
