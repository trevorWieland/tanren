"""Tests for GCP Secret Manager SecretProvider."""

from __future__ import annotations

from types import SimpleNamespace
from unittest.mock import Mock

import pytest

from tanren_core.adapters.gcp_secret_manager import GCPSecretManagerProvider


def _build_secretmanager_module(client):
    """Build a fake google.cloud.secretmanager module."""
    return SimpleNamespace(
        SecretManagerServiceClient=Mock(return_value=client),
    )


def _make_provider(monkeypatch, client, **overrides):
    """Create a GCPSecretManagerProvider with mocked SDK."""
    mod = _build_secretmanager_module(client)
    monkeypatch.setattr(
        "tanren_core.adapters.gcp_secret_manager._import_secret_manager",
        lambda: mod,
    )
    return GCPSecretManagerProvider(
        project_id=overrides.get("project_id", "my-project"),
    )


@pytest.mark.asyncio
class TestGCPSecretManagerProvider:
    async def test_get_returns_value(self, monkeypatch):
        response = SimpleNamespace(payload=SimpleNamespace(data=b"super-secret-value"))
        client = Mock()
        client.access_secret_version = Mock(return_value=response)

        provider = _make_provider(monkeypatch, client)
        value = await provider.get_secret("my-secret")

        assert value == "super-secret-value"
        client.access_secret_version.assert_called_once()
        call_kwargs = client.access_secret_version.call_args.kwargs
        assert "my-project" in call_kwargs["request"]["name"]
        assert "my-secret" in call_kwargs["request"]["name"]
        assert "latest" in call_kwargs["request"]["name"]

    async def test_get_with_custom_version(self, monkeypatch):
        response = SimpleNamespace(payload=SimpleNamespace(data=b"v3-value"))
        client = Mock()
        client.access_secret_version = Mock(return_value=response)

        provider = _make_provider(monkeypatch, client)
        value = await provider.get_secret("key", version="3")

        assert value == "v3-value"
        call_kwargs = client.access_secret_version.call_args.kwargs
        assert "/versions/3" in call_kwargs["request"]["name"]

    async def test_get_caches_result(self, monkeypatch):
        response = SimpleNamespace(payload=SimpleNamespace(data=b"cached-value"))
        client = Mock()
        client.access_secret_version = Mock(return_value=response)

        provider = _make_provider(monkeypatch, client)
        v1 = await provider.get_secret("key")
        v2 = await provider.get_secret("key")

        assert v1 == v2 == "cached-value"
        assert client.access_secret_version.call_count == 1

    async def test_get_returns_none_on_error(self, monkeypatch):
        client = Mock()
        client.access_secret_version = Mock(side_effect=RuntimeError("API error"))

        provider = _make_provider(monkeypatch, client)
        value = await provider.get_secret("missing-secret")

        assert value is None

    async def test_list_secrets(self, monkeypatch):
        secret1 = SimpleNamespace(name="projects/my-project/secrets/secret-a")
        secret2 = SimpleNamespace(name="projects/my-project/secrets/secret-b")
        client = Mock()
        client.list_secrets = Mock(return_value=[secret1, secret2])

        provider = _make_provider(monkeypatch, client)
        result = await provider.list_secrets()

        assert result == ["secret-a", "secret-b"]

    async def test_list_secrets_returns_empty_on_error(self, monkeypatch):
        client = Mock()
        client.list_secrets = Mock(side_effect=RuntimeError("API error"))

        provider = _make_provider(monkeypatch, client)
        result = await provider.list_secrets()

        assert result == []


def test_import_error_clear_message(monkeypatch):
    def _raise():
        raise ImportError(
            "google-cloud-secret-manager is required for GCP secrets. "
            "Install it with: uv sync --extra gcp"
        )

    monkeypatch.setattr(
        "tanren_core.adapters.gcp_secret_manager._import_secret_manager",
        _raise,
    )

    with pytest.raises(ImportError, match="uv sync --extra gcp"):
        GCPSecretManagerProvider(project_id="test")
