"""Tests for DotenvSecretProvider."""

from typing import TYPE_CHECKING

import pytest

from tanren_core.adapters.dotenv_secret_provider import DotenvSecretProvider

if TYPE_CHECKING:
    from pathlib import Path


@pytest.mark.asyncio
class TestDotenvSecretProvider:
    async def test_get_from_secrets_env(self, tmp_path: Path):
        (tmp_path / "secrets.env").write_text("API_KEY=sk-abc123\n")
        provider = DotenvSecretProvider(secrets_dir=tmp_path)
        assert await provider.get_secret("API_KEY") == "sk-abc123"

    async def test_get_from_secrets_d(self, tmp_path: Path):
        (tmp_path / "secrets.env").write_text("")
        sd = tmp_path / "secrets.d"
        sd.mkdir()
        (sd / "api.env").write_text("GCP_KEY=gcp-val\n")
        provider = DotenvSecretProvider(secrets_dir=tmp_path)
        assert await provider.get_secret("GCP_KEY") == "gcp-val"

    async def test_secrets_d_overrides_secrets_env(self, tmp_path: Path):
        (tmp_path / "secrets.env").write_text("K=from-base\n")
        sd = tmp_path / "secrets.d"
        sd.mkdir()
        (sd / "override.env").write_text("K=from-d\n")
        provider = DotenvSecretProvider(secrets_dir=tmp_path)
        assert await provider.get_secret("K") == "from-d"

    async def test_get_returns_none_for_missing_key(self, tmp_path: Path):
        (tmp_path / "secrets.env").write_text("OTHER=val\n")
        provider = DotenvSecretProvider(secrets_dir=tmp_path)
        assert await provider.get_secret("MISSING") is None

    async def test_get_returns_none_for_missing_dir(self, tmp_path: Path):
        provider = DotenvSecretProvider(secrets_dir=tmp_path / "nonexistent")
        assert await provider.get_secret("X") is None

    async def test_list_secrets(self, tmp_path: Path):
        (tmp_path / "secrets.env").write_text("A=1\nB=2\n")
        provider = DotenvSecretProvider(secrets_dir=tmp_path)
        result = await provider.list_secrets()
        assert set(result) == {"A", "B"}

    async def test_list_empty_dir(self, tmp_path: Path):
        provider = DotenvSecretProvider(secrets_dir=tmp_path)
        assert await provider.list_secrets() == []

    async def test_caching(self, tmp_path: Path):
        (tmp_path / "secrets.env").write_text("K=v\n")
        provider = DotenvSecretProvider(secrets_dir=tmp_path)
        await provider.get_secret("K")
        # Modify file after cache
        (tmp_path / "secrets.env").write_text("K=changed\n")
        # Should still return cached value
        assert await provider.get_secret("K") == "v"
