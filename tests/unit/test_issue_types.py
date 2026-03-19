"""Tests for issue data models."""

import pytest
from pydantic import ValidationError

from tanren_core.adapters.issue_types import Issue, IssueSummary


class TestIssue:
    def test_minimal(self):
        issue = Issue(id="144", title="Fix login bug")
        assert issue.id == "144"
        assert issue.title == "Fix login bug"
        assert not issue.body
        assert not issue.status
        assert issue.labels == ()
        assert not issue.url
        assert issue.metadata == {}

    def test_full(self):
        issue = Issue(
            id="PROJ-123",
            title="Add feature X",
            body="Detailed description",
            status="in_progress",
            labels=("bug", "urgent"),
            url="https://linear.app/proj/PROJ-123",
            metadata={"priority": "high"},
        )
        assert issue.id == "PROJ-123"
        assert issue.labels == ("bug", "urgent")
        assert issue.metadata == {"priority": "high"}

    def test_frozen(self):
        issue = Issue(id="1", title="Test")
        with pytest.raises(ValidationError):
            issue.title = "Changed"


class TestIssueSummary:
    def test_minimal(self):
        s = IssueSummary(id="42", title="Quick fix")
        assert s.id == "42"
        assert s.title == "Quick fix"
        assert not s.status
        assert not s.url

    def test_full(self):
        s = IssueSummary(
            id="PROJ-99",
            title="Upgrade deps",
            status="todo",
            url="https://example.com/PROJ-99",
        )
        assert s.status == "todo"
        assert s.url == "https://example.com/PROJ-99"
