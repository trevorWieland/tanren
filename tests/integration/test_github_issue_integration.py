"""Integration tests for GitHub issue source — requires real token.

Run with:
    uv run pytest tests/integration/test_github_issue_integration.py -v --timeout=60
"""

from __future__ import annotations

import os

import pytest

from tanren_core.adapters.github_issue import GitHubIssueSettings, GitHubIssueSource

pytestmark = pytest.mark.github


@pytest.fixture()
def source():
    """Create a GitHubIssueSource using real GitHub credentials."""
    if not os.environ.get("GITHUB_TOKEN"):
        raise pytest.skip.Exception("GITHUB_TOKEN not set")

    settings = GitHubIssueSettings(owner="trevorWieland", repo="tanren")
    return GitHubIssueSource(settings)


class TestGitHubIssueSourceIntegration:
    async def test_get_issue(self, source):
        """Fetch a real GitHub issue."""
        issue = await source.get_issue("14")
        assert issue.id == "14"
        assert issue.title
        assert issue.url

    async def test_list_issues(self, source):
        """List issues from a real repo."""
        summaries = await source.list_issues(status="open")
        assert isinstance(summaries, list)
