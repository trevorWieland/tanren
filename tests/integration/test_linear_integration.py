"""Integration tests for Linear issue source -- requires real token.

Run with:
    uv run pytest tests/integration/test_linear_integration.py -v --timeout=60
"""

from __future__ import annotations

import os

import pytest

from tanren_core.adapters.linear_issue import LinearIssueSettings, LinearIssueSource

pytestmark = pytest.mark.linear


@pytest.fixture()
def source():
    """Create a LinearIssueSource using real Linear credentials."""
    if not os.environ.get("LINEAR_API_KEY"):
        raise pytest.skip.Exception("LINEAR_API_KEY not set")

    team_key = os.environ.get("LINEAR_TEAM_KEY", "")
    if not team_key:
        raise pytest.skip.Exception("LINEAR_TEAM_KEY not set")

    settings = LinearIssueSettings(team_key=team_key)
    return LinearIssueSource(settings)


class TestLinearIssueSourceIntegration:
    async def test_get_issue(self, source):
        """Fetch a real Linear issue."""
        ...

    async def test_list_issues(self, source):
        """List issues from a real Linear team."""
        summaries = await source.list_issues()
        assert isinstance(summaries, list)
