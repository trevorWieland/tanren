"""GitHub issue source adapter using GraphQL API."""

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

_GITHUB_GRAPHQL_URL = "https://api.github.com/graphql"


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
            "httpx is required for the GitHub issue adapter. "
            "Install it with: uv sync --extra github"
        ) from None


class GitHubIssueSettings(BaseModel):
    """Configuration for GitHub issue source."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    token_env: str = Field(
        default="GITHUB_TOKEN",
        description="Environment variable name containing the GitHub token",
    )
    owner: str = Field(..., description="Repository owner (user or org)")
    repo: str = Field(..., description="Repository name")
    api_url: str = Field(
        default=_GITHUB_GRAPHQL_URL,
        description="GitHub GraphQL API endpoint (override for GitHub Enterprise)",
    )

    @classmethod
    def from_settings(cls, settings: Mapping[str, JsonValue]) -> GitHubIssueSettings:
        """Parse from tanren.yml issue_source.settings.

        Returns:
            Validated GitHubIssueSettings.
        """
        return cls.model_validate(settings)


_GET_ISSUE_QUERY = """
query($owner: String!, $repo: String!, $number: Int!) {
  repository(owner: $owner, name: $repo) {
    issue(number: $number) {
      id
      number
      title
      body
      state
      url
      labels(first: 20) { nodes { name } }
      milestone { title }
      trackedInIssues(first: 10) { nodes { number title } }
      trackedIssues(first: 10) { nodes { number title } }
    }
  }
}
"""

_LIST_ISSUES_QUERY = """
query($owner: String!, $repo: String!, $labels: [String!], $states: [IssueState!]) {
  repository(owner: $owner, name: $repo) {
    issues(first: 50, labels: $labels, states: $states,
           orderBy: {field: CREATED_AT, direction: DESC}) {
      nodes {
        number
        title
        state
        url
        labels(first: 10) { nodes { name } }
      }
    }
  }
}
"""

_ADD_COMMENT_MUTATION = """
mutation($issueId: ID!, $body: String!) {
  addComment(input: { subjectId: $issueId, body: $body }) {
    commentEdge { node { id } }
  }
}
"""

_CLOSE_ISSUE_MUTATION = """
mutation($issueId: ID!, $stateReason: IssueClosedStateReason) {
  closeIssue(input: { issueId: $issueId, stateReason: $stateReason }) {
    issue { number state }
  }
}
"""

_REOPEN_ISSUE_MUTATION = """
mutation($issueId: ID!) {
  reopenIssue(input: { issueId: $issueId }) {
    issue { number state }
  }
}
"""

_CLOSE_STATUSES = frozenset({"closed", "merged", "done", "completed"})
_OPEN_STATUSES = frozenset({"open", "reopened"})

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


class GitHubIssueSource:
    """Fetch and update GitHub issues via GraphQL API."""

    def __init__(self, settings: GitHubIssueSettings) -> None:
        """Initialize with GitHub issue settings.

        Args:
            settings: Validated GitHub issue settings.

        Raises:
            ValueError: If the token environment variable is not set.
        """
        token = os.environ.get(settings.token_env)
        if not token:
            raise ValueError(f"Missing GitHub token in environment variable: {settings.token_env}")
        httpx_mod = _import_httpx()
        self._client: httpx.Client = httpx_mod.Client(
            headers={"Authorization": f"Bearer {token}"},
            timeout=30.0,
        )
        self._api_url = settings.api_url
        self._owner = settings.owner
        self._repo = settings.repo

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
        node_id = str(data.get("id", ""))
        number = data.get("number", 0)
        title = str(data.get("title", ""))
        body = str(data.get("body", ""))
        state = str(data.get("state", ""))
        url = str(data.get("url", ""))

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

        milestone_title = ""
        milestone_data = data.get("milestone")
        if isinstance(milestone_data, dict):
            mt = _as_dict(milestone_data).get("title")
            if isinstance(mt, str):
                milestone_title = mt

        blocked_by_numbers: list[str] = []
        tracked_in = data.get("trackedInIssues")
        if isinstance(tracked_in, dict):
            nodes = _as_dict(tracked_in).get("nodes")
            if isinstance(nodes, list):
                for node in nodes:
                    if isinstance(node, dict):
                        n = _as_dict(node).get("number")
                        if n is not None:
                            blocked_by_numbers.append(str(n))

        blocking_numbers: list[str] = []
        tracked = data.get("trackedIssues")
        if isinstance(tracked, dict):
            nodes = _as_dict(tracked).get("nodes")
            if isinstance(nodes, list):
                for node in nodes:
                    if isinstance(node, dict):
                        n = _as_dict(node).get("number")
                        if n is not None:
                            blocking_numbers.append(str(n))

        metadata: dict[str, str] = {"node_id": node_id}
        if milestone_title:
            metadata["milestone"] = milestone_title
        if blocked_by_numbers:
            metadata["blocked_by"] = ",".join(blocked_by_numbers)
        if blocking_numbers:
            metadata["blocking"] = ",".join(blocking_numbers)

        return Issue(
            id=str(number),
            title=title,
            body=body,
            status=state,
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
        number = data.get("number", 0)
        title = str(data.get("title", ""))
        state = str(data.get("state", ""))
        url = str(data.get("url", ""))
        return IssueSummary(id=str(number), title=title, status=state, url=url)

    async def get_issue(self, issue_id: str) -> Issue:
        """Fetch full issue detail by ID.

        Args:
            issue_id: Numeric GitHub issue number as a string.

        Returns:
            The Issue model for the requested issue.

        Raises:
            ValueError: If issue_id is not numeric or the issue is not found.
        """
        if not issue_id.isdigit():
            raise ValueError(f"GitHub issue IDs must be numeric, got: {issue_id!r}")
        variables: dict[str, object] = {
            "owner": self._owner,
            "repo": self._repo,
            "number": int(issue_id),
        }
        data = await asyncio.to_thread(self._execute, _GET_ISSUE_QUERY, variables)
        repo = data.get("repository")
        if not isinstance(repo, dict):
            raise ValueError(f"Issue {issue_id} not found")
        issue_data = _as_dict(repo).get("issue")
        if not isinstance(issue_data, dict):
            raise ValueError(f"Issue {issue_id} not found")
        return self._build_issue(_as_dict(issue_data))

    async def list_issues(
        self, *, project: str | None = None, status: str | None = None
    ) -> list[IssueSummary]:
        """List issues, optionally filtered by status.

        Args:
            project: Ignored for GitHub (repo-scoped via settings).
            status: Filter by status: ``"open"`` or ``"closed"``. None returns all.

        Returns:
            List of IssueSummary instances.
        """
        variables: dict[str, object] = {
            "owner": self._owner,
            "repo": self._repo,
        }
        if status is not None:
            normalized = status.strip().upper()
            if normalized in ("OPEN", "CLOSED"):
                variables["states"] = [normalized]
            else:
                logger.warning(
                    "Unsupported GitHub issue status %r; returning all issues without state filter",
                    status,
                )

        data = await asyncio.to_thread(self._execute, _LIST_ISSUES_QUERY, variables)
        repo = data.get("repository")
        if not isinstance(repo, dict):
            return []
        issues_data = _as_dict(repo).get("issues")
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

        GitHub only supports OPEN and CLOSED states. Granular statuses are
        logged as warnings without raising.

        Args:
            issue_id: Numeric GitHub issue number as a string.
            status: Target status string.
        """
        normalized = status.strip().lower()
        issue = await self.get_issue(issue_id)
        node_id = issue.metadata.get("node_id", "")

        if normalized in _CLOSE_STATUSES:
            variables: dict[str, object] = {
                "issueId": node_id,
                "stateReason": "COMPLETED",
            }
            await asyncio.to_thread(self._execute, _CLOSE_ISSUE_MUTATION, variables)
        elif normalized in _OPEN_STATUSES:
            reopen_vars: dict[str, object] = {"issueId": node_id}
            await asyncio.to_thread(self._execute, _REOPEN_ISSUE_MUTATION, reopen_vars)
        else:
            logger.warning(
                "GitHub does not support granular status %r for issue %s; skipping",
                status,
                issue_id,
            )

    async def add_comment(self, issue_id: str, body: str) -> None:
        """Append a comment to an issue.

        Args:
            issue_id: Numeric GitHub issue number as a string.
            body: Comment body text.
        """
        issue = await self.get_issue(issue_id)
        node_id = issue.metadata.get("node_id", "")
        variables: dict[str, object] = {"issueId": node_id, "body": body}
        await asyncio.to_thread(self._execute, _ADD_COMMENT_MUTATION, variables)
