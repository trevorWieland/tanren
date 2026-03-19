"""Tests for GitHub issue source adapter."""

from __future__ import annotations

import logging
import types
from unittest.mock import Mock

import pytest

from tanren_core.adapters.github_issue import GitHubIssueSettings, GitHubIssueSource
from tanren_core.adapters.issue_types import Issue, IssueSummary


class _FakeResponse:
    def __init__(self, data, status_code=200):
        self.status_code = status_code
        self._json = data

    def json(self):
        return self._json

    def raise_for_status(self):
        if self.status_code >= 400:
            raise RuntimeError(f"HTTP {self.status_code}")


def _settings(**overrides):
    defaults = {"owner": "testorg", "repo": "testrepo"}
    defaults.update(overrides)
    return GitHubIssueSettings(**defaults)


def _make_source(monkeypatch, mock_client, **settings_kw):
    monkeypatch.setenv("GITHUB_TOKEN", "ghp_test_token")
    httpx_mod = types.SimpleNamespace(Client=Mock(return_value=mock_client))
    monkeypatch.setattr(
        "tanren_core.adapters.github_issue._import_httpx",
        lambda: httpx_mod,
    )
    return GitHubIssueSource(_settings(**settings_kw))


def _issue_graphql_response(
    *,
    number=42,
    node_id="I_kwDOtest",
    title="Test issue",
    body="Issue body",
    state="OPEN",
    url="https://github.com/testorg/testrepo/issues/42",
    labels=None,
    milestone=None,
    tracked_in=None,
    tracked=None,
):
    return {
        "data": {
            "repository": {
                "issue": {
                    "id": node_id,
                    "number": number,
                    "title": title,
                    "body": body,
                    "state": state,
                    "url": url,
                    "labels": {"nodes": [{"name": n} for n in (labels or [])]},
                    "milestone": {"title": milestone} if milestone else None,
                    "trackedInIssues": {
                        "nodes": [{"number": n, "title": f"#{n}"} for n in (tracked_in or [])]
                    },
                    "trackedIssues": {
                        "nodes": [{"number": n, "title": f"#{n}"} for n in (tracked or [])]
                    },
                }
            }
        }
    }


def _list_issues_graphql_response(issues=None):
    if issues is None:
        issues = [
            {
                "number": 1,
                "title": "First",
                "state": "OPEN",
                "url": "https://example.com/1",
                "labels": {"nodes": []},
            },
            {
                "number": 2,
                "title": "Second",
                "state": "CLOSED",
                "url": "https://example.com/2",
                "labels": {"nodes": []},
            },
        ]
    return {"data": {"repository": {"issues": {"nodes": issues}}}}


class TestGitHubIssueSettings:
    def test_from_settings(self):
        raw = {"owner": "myorg", "repo": "myrepo", "token_env": "MY_TOKEN"}
        settings = GitHubIssueSettings.from_settings(raw)
        assert settings.owner == "myorg"
        assert settings.repo == "myrepo"
        assert settings.token_env == "MY_TOKEN"

    def test_defaults(self):
        settings = _settings()
        assert settings.token_env == "GITHUB_TOKEN"
        assert settings.api_url == "https://api.github.com/graphql"

    def test_github_enterprise_url(self):
        settings = _settings(api_url="https://github.corp.com/api/graphql")
        assert settings.api_url == "https://github.corp.com/api/graphql"


class TestGitHubIssueSourceInit:
    def test_missing_token_raises(self, monkeypatch):
        monkeypatch.delenv("GITHUB_TOKEN", raising=False)
        httpx_mod = types.SimpleNamespace(Client=Mock())
        monkeypatch.setattr(
            "tanren_core.adapters.github_issue._import_httpx",
            lambda: httpx_mod,
        )
        with pytest.raises(ValueError, match="Missing GitHub token"):
            GitHubIssueSource(_settings())

    def test_missing_httpx_raises(self, monkeypatch):
        monkeypatch.setenv("GITHUB_TOKEN", "ghp_test")

        def _raise():
            raise ImportError(
                "httpx is required for the GitHub issue adapter. "
                "Install it with: uv sync --extra github"
            )

        monkeypatch.setattr(
            "tanren_core.adapters.github_issue._import_httpx",
            _raise,
        )
        with pytest.raises(ImportError, match="uv sync --extra github"):
            GitHubIssueSource(_settings())


class TestGetIssue:
    async def test_returns_issue_model(self, monkeypatch):
        response_data = _issue_graphql_response(
            number=42,
            node_id="I_kwDOtest42",
            title="Test issue",
            body="Issue body",
            state="OPEN",
            labels=["bug", "priority"],
            milestone="v1.0",
            tracked_in=[10, 20],
            tracked=[30],
        )
        client = Mock()
        client.post = Mock(return_value=_FakeResponse(response_data))
        source = _make_source(monkeypatch, client)

        issue = await source.get_issue("42")

        assert isinstance(issue, Issue)
        assert issue.id == "42"
        assert issue.title == "Test issue"
        assert issue.body == "Issue body"
        assert issue.status == "OPEN"
        assert issue.labels == ("bug", "priority")
        assert issue.url == "https://github.com/testorg/testrepo/issues/42"
        assert issue.metadata["node_id"] == "I_kwDOtest42"
        assert issue.metadata["milestone"] == "v1.0"
        assert issue.metadata["blocked_by"] == "10,20"
        assert issue.metadata["blocking"] == "30"

    async def test_not_found_raises(self, monkeypatch):
        response_data = {"data": {"repository": {"issue": None}}}
        client = Mock()
        client.post = Mock(return_value=_FakeResponse(response_data))
        source = _make_source(monkeypatch, client)

        with pytest.raises(ValueError, match="not found"):
            await source.get_issue("999")

    async def test_non_numeric_id_raises(self, monkeypatch):
        client = Mock()
        source = _make_source(monkeypatch, client)

        with pytest.raises(ValueError, match="numeric"):
            await source.get_issue("PROJ-123")


class TestListIssues:
    async def test_returns_summaries(self, monkeypatch):
        response_data = _list_issues_graphql_response()
        client = Mock()
        client.post = Mock(return_value=_FakeResponse(response_data))
        source = _make_source(monkeypatch, client)

        summaries = await source.list_issues()

        assert len(summaries) == 2
        assert all(isinstance(s, IssueSummary) for s in summaries)
        assert summaries[0].id == "1"
        assert summaries[0].title == "First"
        assert summaries[0].status == "OPEN"
        assert summaries[1].id == "2"
        assert summaries[1].status == "CLOSED"

    async def test_status_filter(self, monkeypatch):
        response_data = _list_issues_graphql_response()
        client = Mock()
        client.post = Mock(return_value=_FakeResponse(response_data))
        source = _make_source(monkeypatch, client)

        await source.list_issues(status="open")

        call_kwargs = client.post.call_args.kwargs
        json_body = call_kwargs["json"]
        assert json_body["variables"]["states"] == ["OPEN"]

    async def test_defaults(self, monkeypatch):
        response_data = _list_issues_graphql_response()
        client = Mock()
        client.post = Mock(return_value=_FakeResponse(response_data))
        source = _make_source(monkeypatch, client)

        await source.list_issues()

        call_kwargs = client.post.call_args.kwargs
        json_body = call_kwargs["json"]
        assert "states" not in json_body["variables"]


class TestUpdateStatus:
    async def test_closes_issue(self, monkeypatch):
        get_response = _issue_graphql_response(number=42, node_id="I_node42")
        close_response = {"data": {"closeIssue": {"issue": {"number": 42, "state": "CLOSED"}}}}
        call_count = 0

        def _side_effect(*_args, **kwargs):
            nonlocal call_count
            call_count += 1
            json_body = kwargs.get("json", {})
            query = json_body.get("query", "")
            if "closeIssue" in query:
                return _FakeResponse(close_response)
            return _FakeResponse(get_response)

        client = Mock()
        client.post = Mock(side_effect=_side_effect)
        source = _make_source(monkeypatch, client)

        await source.update_status("42", "closed")

        assert call_count == 2  # get_issue + closeIssue

    async def test_reopens_issue(self, monkeypatch):
        get_response = _issue_graphql_response(number=42, node_id="I_node42", state="CLOSED")
        reopen_response = {"data": {"reopenIssue": {"issue": {"number": 42, "state": "OPEN"}}}}

        def _side_effect(*_args, **kwargs):
            json_body = kwargs.get("json", {})
            query = json_body.get("query", "")
            if "reopenIssue" in query:
                return _FakeResponse(reopen_response)
            return _FakeResponse(get_response)

        client = Mock()
        client.post = Mock(side_effect=_side_effect)
        source = _make_source(monkeypatch, client)

        await source.update_status("42", "open")

    async def test_unmappable_warns(self, monkeypatch, caplog):
        get_response = _issue_graphql_response(number=42, node_id="I_node42")
        client = Mock()
        client.post = Mock(return_value=_FakeResponse(get_response))
        source = _make_source(monkeypatch, client)

        with caplog.at_level(logging.WARNING):
            await source.update_status("42", "executing")

        assert "granular status" in caplog.text.lower() or "executing" in caplog.text


class TestAddComment:
    async def test_resolves_node_id_and_posts(self, monkeypatch):
        get_response = _issue_graphql_response(number=42, node_id="I_node42")
        comment_response = {"data": {"addComment": {"commentEdge": {"node": {"id": "IC_abc"}}}}}
        call_count = 0

        def _side_effect(*_args, **kwargs):
            nonlocal call_count
            call_count += 1
            json_body = kwargs.get("json", {})
            query = json_body.get("query", "")
            if "addComment" in query:
                return _FakeResponse(comment_response)
            return _FakeResponse(get_response)

        client = Mock()
        client.post = Mock(side_effect=_side_effect)
        source = _make_source(monkeypatch, client)

        await source.add_comment("42", "Test comment")

        assert call_count == 2
        # Verify the comment mutation was called with the correct node_id
        last_call = client.post.call_args_list[-1]
        json_body = last_call.kwargs["json"]
        assert json_body["variables"]["issueId"] == "I_node42"
        assert json_body["variables"]["body"] == "Test comment"
