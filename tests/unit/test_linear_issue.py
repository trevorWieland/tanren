"""Tests for Linear issue source adapter."""

from __future__ import annotations

import logging
import types
from unittest.mock import Mock

import pytest

from tanren_core.adapters.issue_types import Issue, IssueSummary
from tanren_core.adapters.linear_issue import LinearIssueSettings, LinearIssueSource


class _FakeResponse:
    def __init__(self, data, status_code=200):
        self.status_code = status_code
        self._json = data

    def json(self):
        return self._json

    def raise_for_status(self):
        if self.status_code >= 400:
            raise RuntimeError(f"HTTP {self.status_code}")


def _settings(**overrides: object) -> LinearIssueSettings:
    defaults: dict[str, object] = {"team_key": "PROJ"}
    defaults.update(overrides)
    return LinearIssueSettings.model_validate(defaults)


def _make_source(monkeypatch, mock_client, **settings_kw):
    monkeypatch.setenv("LINEAR_API_KEY", "lin_test_key")
    httpx_mod = types.SimpleNamespace(Client=Mock(return_value=mock_client))
    monkeypatch.setattr(
        "tanren_core.adapters.linear_issue._import_httpx",
        lambda: httpx_mod,
    )
    return LinearIssueSource(_settings(**settings_kw))


def _issue_graphql_response(
    *,
    linear_id="uuid-abc-123",
    identifier="PROJ-42",
    title="Test issue",
    description="Issue body",
    state_name="In Progress",
    state_type="started",
    state_id="state-uuid-1",
    labels=None,
    url="https://linear.app/proj/issue/PROJ-42",
    team_id="team-uuid",
    team_key="PROJ",
):
    return {
        "data": {
            "issue": {
                "id": linear_id,
                "identifier": identifier,
                "title": title,
                "description": description,
                "state": {"id": state_id, "name": state_name, "type": state_type},
                "labels": {"nodes": [{"name": n} for n in (labels or [])]},
                "url": url,
                "team": {"id": team_id, "key": team_key},
            }
        }
    }


def _list_issues_graphql_response(issues=None):
    if issues is None:
        issues = [
            {
                "identifier": "PROJ-1",
                "title": "First",
                "state": {"name": "Todo"},
                "url": "https://linear.app/proj/issue/PROJ-1",
            },
            {
                "identifier": "PROJ-2",
                "title": "Second",
                "state": {"name": "Done"},
                "url": "https://linear.app/proj/issue/PROJ-2",
            },
        ]
    return {"data": {"issues": {"nodes": issues}}}


def _team_states_response(states=None):
    if states is None:
        states = [
            {"id": "state-backlog", "name": "Backlog", "type": "backlog"},
            {"id": "state-todo", "name": "Todo", "type": "unstarted"},
            {"id": "state-inprog", "name": "In Progress", "type": "started"},
            {"id": "state-done", "name": "Done", "type": "completed"},
            {"id": "state-cancelled", "name": "Cancelled", "type": "cancelled"},
        ]
    return {"data": {"team": {"states": {"nodes": states}}}}


class TestLinearIssueSettings:
    def test_from_settings(self):
        raw = {"team_key": "ENG", "api_key_env": "MY_LINEAR_KEY"}
        settings = LinearIssueSettings.from_settings(raw)
        assert settings.team_key == "ENG"
        assert settings.api_key_env == "MY_LINEAR_KEY"

    def test_defaults(self):
        settings = _settings()
        assert settings.api_key_env == "LINEAR_API_KEY"
        assert settings.api_url == "https://api.linear.app/graphql"
        assert "executing" in settings.status_mapping
        assert settings.status_mapping["executing"] == "In Progress"

    def test_custom_status_mapping(self):
        mapping = {"todo": "Backlog", "done": "Complete"}
        settings = _settings(status_mapping=mapping)
        assert settings.status_mapping == mapping

    def test_custom_api_url(self):
        settings = _settings(api_url="https://linear.corp.com/graphql")
        assert settings.api_url == "https://linear.corp.com/graphql"


class TestLinearIssueSourceInit:
    def test_missing_api_key_raises(self, monkeypatch):
        monkeypatch.delenv("LINEAR_API_KEY", raising=False)
        httpx_mod = types.SimpleNamespace(Client=Mock())
        monkeypatch.setattr(
            "tanren_core.adapters.linear_issue._import_httpx",
            lambda: httpx_mod,
        )
        with pytest.raises(ValueError, match="Missing Linear API key"):
            LinearIssueSource(_settings())

    def test_missing_httpx_raises(self, monkeypatch):
        monkeypatch.setenv("LINEAR_API_KEY", "lin_test")

        def _raise():
            raise ImportError(
                "httpx is required for the Linear issue adapter. "
                "Install it with: uv sync --extra linear"
            )

        monkeypatch.setattr(
            "tanren_core.adapters.linear_issue._import_httpx",
            _raise,
        )
        with pytest.raises(ImportError, match="uv sync --extra linear"):
            LinearIssueSource(_settings())


class TestGetIssue:
    async def test_returns_issue_model(self, monkeypatch):
        response_data = _issue_graphql_response(
            linear_id="uuid-abc-123",
            identifier="PROJ-42",
            title="Test issue",
            description="Issue body",
            state_name="In Progress",
            state_type="started",
            labels=["bug", "priority"],
            team_id="team-uuid",
            team_key="PROJ",
        )
        client = Mock()
        client.post = Mock(return_value=_FakeResponse(response_data))
        source = _make_source(monkeypatch, client)

        issue = await source.get_issue("PROJ-42")

        assert isinstance(issue, Issue)
        assert issue.id == "PROJ-42"
        assert issue.title == "Test issue"
        assert issue.body == "Issue body"
        assert issue.status == "In Progress"
        assert issue.labels == ("bug", "priority")
        assert issue.url == "https://linear.app/proj/issue/PROJ-42"
        assert issue.metadata["linear_id"] == "uuid-abc-123"
        assert issue.metadata["team_key"] == "PROJ"
        assert issue.metadata["team_id"] == "team-uuid"
        assert issue.metadata["state_type"] == "started"

    async def test_not_found_raises(self, monkeypatch):
        response_data = {"data": {"issue": None}}
        client = Mock()
        client.post = Mock(return_value=_FakeResponse(response_data))
        source = _make_source(monkeypatch, client)

        with pytest.raises(TypeError, match="not found"):
            await source.get_issue("PROJ-999")

    async def test_caches_team_id(self, monkeypatch):
        response_data = _issue_graphql_response(team_id="team-uuid-42")
        client = Mock()
        client.post = Mock(return_value=_FakeResponse(response_data))
        source = _make_source(monkeypatch, client)

        assert source._team_id is None
        await source.get_issue("PROJ-42")
        assert source._team_id == "team-uuid-42"


class TestListIssues:
    async def test_returns_summaries(self, monkeypatch):
        response_data = _list_issues_graphql_response()
        client = Mock()
        client.post = Mock(return_value=_FakeResponse(response_data))
        source = _make_source(monkeypatch, client)

        summaries = await source.list_issues()

        assert len(summaries) == 2
        assert all(isinstance(s, IssueSummary) for s in summaries)
        assert summaries[0].id == "PROJ-1"
        assert summaries[0].title == "First"
        assert summaries[0].status == "Todo"
        assert summaries[1].id == "PROJ-2"
        assert summaries[1].status == "Done"

    async def test_team_key_filter(self, monkeypatch):
        response_data = _list_issues_graphql_response()
        client = Mock()
        client.post = Mock(return_value=_FakeResponse(response_data))
        source = _make_source(monkeypatch, client)

        await source.list_issues()

        call_kwargs = client.post.call_args.kwargs
        json_body = call_kwargs["json"]
        assert json_body["variables"]["teamKey"] == "PROJ"

    async def test_status_filter(self, monkeypatch):
        response_data = _list_issues_graphql_response()
        client = Mock()
        client.post = Mock(return_value=_FakeResponse(response_data))
        source = _make_source(monkeypatch, client)

        await source.list_issues(status="In Progress")

        call_kwargs = client.post.call_args.kwargs
        json_body = call_kwargs["json"]
        assert json_body["variables"]["stateName"] == "In Progress"

    async def test_project_overrides_team_key(self, monkeypatch):
        response_data = _list_issues_graphql_response()
        client = Mock()
        client.post = Mock(return_value=_FakeResponse(response_data))
        source = _make_source(monkeypatch, client)

        await source.list_issues(project="OTHER")

        call_kwargs = client.post.call_args.kwargs
        json_body = call_kwargs["json"]
        assert json_body["variables"]["teamKey"] == "OTHER"


class TestUpdateStatus:
    async def test_maps_tanren_status_to_linear_state(self, monkeypatch):
        get_response = _issue_graphql_response(team_id="team-uuid")
        states_response = _team_states_response()
        update_response = {
            "data": {"issueUpdate": {"success": True, "issue": {"state": {"name": "In Progress"}}}}
        }

        def _side_effect(*_args, **kwargs):
            json_body = kwargs.get("json", {})
            query = json_body.get("query", "")
            if "issueUpdate" in query:
                return _FakeResponse(update_response)
            if "states" in query:
                return _FakeResponse(states_response)
            return _FakeResponse(get_response)

        client = Mock()
        client.post = Mock(side_effect=_side_effect)
        source = _make_source(monkeypatch, client)

        await source.update_status("PROJ-42", "executing")

        # Verify the update mutation was called with the correct state UUID.
        last_call = client.post.call_args_list[-1]
        json_body = last_call.kwargs["json"]
        assert json_body["variables"]["stateId"] == "state-inprog"
        assert json_body["variables"]["id"] == "PROJ-42"

    async def test_works_without_prior_get_issue(self, monkeypatch):
        """update_status resolves team context automatically."""
        get_response = _issue_graphql_response(team_id="team-uuid")
        states_response = _team_states_response()
        update_response = {
            "data": {"issueUpdate": {"success": True, "issue": {"state": {"name": "Done"}}}}
        }

        def _side_effect(*_args, **kwargs):
            json_body = kwargs.get("json", {})
            query = json_body.get("query", "")
            if "issueUpdate" in query:
                return _FakeResponse(update_response)
            if "states" in query:
                return _FakeResponse(states_response)
            return _FakeResponse(get_response)

        client = Mock()
        client.post = Mock(side_effect=_side_effect)
        source = _make_source(monkeypatch, client)

        assert source._team_id is None
        await source.update_status("PROJ-42", "merged")

        # team_id should now be populated from the internal get_issue call.
        assert source._team_id == "team-uuid"
        last_call = client.post.call_args_list[-1]
        json_body = last_call.kwargs["json"]
        assert json_body["variables"]["stateId"] == "state-done"

    async def test_caches_workflow_states(self, monkeypatch):
        get_response = _issue_graphql_response(team_id="team-uuid")
        states_response = _team_states_response()
        update_response = {
            "data": {"issueUpdate": {"success": True, "issue": {"state": {"name": "Done"}}}}
        }

        states_call_count = 0

        def _side_effect(*_args, **kwargs):
            nonlocal states_call_count
            json_body = kwargs.get("json", {})
            query = json_body.get("query", "")
            if "issueUpdate" in query:
                return _FakeResponse(update_response)
            if "states" in query:
                states_call_count += 1
                return _FakeResponse(states_response)
            return _FakeResponse(get_response)

        client = Mock()
        client.post = Mock(side_effect=_side_effect)
        source = _make_source(monkeypatch, client)

        await source.update_status("PROJ-42", "executing")
        await source.update_status("PROJ-42", "merged")

        # Team states should be fetched only once.
        assert states_call_count == 1

    async def test_unmapped_status_warns(self, monkeypatch, caplog):
        client = Mock()
        source = _make_source(monkeypatch, client)

        with caplog.at_level(logging.WARNING):
            await source.update_status("PROJ-42", "unknown_status")

        assert "no linear status mapping" in caplog.text.lower() or "unknown_status" in caplog.text
        # No GraphQL calls should have been made.
        client.post.assert_not_called()

    async def test_state_name_not_found_warns(self, monkeypatch, caplog):
        get_response = _issue_graphql_response(team_id="team-uuid")
        states_response = _team_states_response(
            states=[
                {"id": "state-only", "name": "OnlyState", "type": "started"},
            ]
        )

        def _side_effect(*_args, **kwargs):
            json_body = kwargs.get("json", {})
            query = json_body.get("query", "")
            if "states" in query:
                return _FakeResponse(states_response)
            return _FakeResponse(get_response)

        client = Mock()
        client.post = Mock(side_effect=_side_effect)
        source = _make_source(
            monkeypatch,
            client,
            status_mapping={"executing": "Nonexistent State"},
        )

        with caplog.at_level(logging.WARNING):
            await source.update_status("PROJ-42", "executing")

        assert "not found" in caplog.text.lower() or "Nonexistent State" in caplog.text


class TestAddComment:
    async def test_resolves_uuid_and_posts(self, monkeypatch):
        get_response = _issue_graphql_response(linear_id="uuid-abc-123")
        comment_response = {"data": {"commentCreate": {"success": True}}}
        call_count = 0

        def _side_effect(*_args, **kwargs):
            nonlocal call_count
            call_count += 1
            json_body = kwargs.get("json", {})
            query = json_body.get("query", "")
            if "commentCreate" in query:
                return _FakeResponse(comment_response)
            return _FakeResponse(get_response)

        client = Mock()
        client.post = Mock(side_effect=_side_effect)
        source = _make_source(monkeypatch, client)

        await source.add_comment("PROJ-42", "Test comment")

        assert call_count == 2  # get_issue + commentCreate

    async def test_comment_uses_linear_id_not_identifier(self, monkeypatch):
        get_response = _issue_graphql_response(
            linear_id="uuid-internal-42",
            identifier="PROJ-42",
        )
        comment_response = {"data": {"commentCreate": {"success": True}}}

        def _side_effect(*_args, **kwargs):
            json_body = kwargs.get("json", {})
            query = json_body.get("query", "")
            if "commentCreate" in query:
                return _FakeResponse(comment_response)
            return _FakeResponse(get_response)

        client = Mock()
        client.post = Mock(side_effect=_side_effect)
        source = _make_source(monkeypatch, client)

        await source.add_comment("PROJ-42", "Test comment")

        # Verify the comment mutation used the UUID, not the identifier.
        last_call = client.post.call_args_list[-1]
        json_body = last_call.kwargs["json"]
        assert json_body["variables"]["issueId"] == "uuid-internal-42"
        assert json_body["variables"]["body"] == "Test comment"
