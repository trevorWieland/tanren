"""Integration tests for GCP Secret Manager provider.

Requires GOOGLE_CLOUD_PROJECT env var and Application Default Credentials.
Run with:
    uv run pytest tests/integration/test_gcp_secrets_integration.py -v --timeout=60
"""

import os

import pytest

pytestmark = pytest.mark.gcp

_PROJECT = os.environ.get("GOOGLE_CLOUD_PROJECT")
_TEST_SECRET_NAME = os.environ.get("GCP_TEST_SECRET_NAME", "tanren-test-secret")


@pytest.fixture()
def provider():
    """Create a live GCP Secret Manager provider."""
    if not _PROJECT:
        pytest.skip("GOOGLE_CLOUD_PROJECT not set")  # type: ignore[invalid-argument-type,too-many-positional-arguments]
    from tanren_core.adapters.gcp_secret_manager import GCPSecretManagerProvider  # noqa: PLC0415

    return GCPSecretManagerProvider(project_id=_PROJECT)


@pytest.mark.asyncio
class TestGCPSecretManagerLive:
    async def test_get_existing_secret(self, provider):
        result = await provider.get_secret(_TEST_SECRET_NAME)
        assert result is not None
        assert len(result) > 0

    async def test_get_nonexistent_returns_none(self, provider):
        result = await provider.get_secret("tanren-nonexistent-secret-xyz-12345")
        assert result is None

    async def test_list_secrets(self, provider):
        result = await provider.list_secrets()
        assert isinstance(result, list)
