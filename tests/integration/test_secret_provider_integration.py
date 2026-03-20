"""Integration tests for SecretProvider implementations with real file I/O."""

from typing import TYPE_CHECKING

import pytest

from tanren_core.adapters.dotenv_secret_provider import DotenvSecretProvider
from tanren_core.env.schema import SecretsConfig, SecretsProviderType
from tanren_core.env.secret_provider_factory import create_secret_provider

if TYPE_CHECKING:
    from pathlib import Path


@pytest.mark.asyncio
class TestDotenvSecretProviderIntegration:
    """End-to-end tests for DotenvSecretProvider with real filesystem."""

    async def test_get_from_secrets_env(self, tmp_path: Path):
        (tmp_path / "secrets.env").write_text("API_KEY=sk-abc123\nDB_URL=postgres://localhost\n")
        provider = DotenvSecretProvider(secrets_dir=tmp_path)
        assert await provider.get_secret("API_KEY") == "sk-abc123"
        assert await provider.get_secret("DB_URL") == "postgres://localhost"

    async def test_get_missing_key_returns_none(self, tmp_path: Path):
        (tmp_path / "secrets.env").write_text("OTHER=val\n")
        provider = DotenvSecretProvider(secrets_dir=tmp_path)
        assert await provider.get_secret("MISSING") is None

    async def test_get_from_secrets_d_directory(self, tmp_path: Path):
        (tmp_path / "secrets.env").write_text("")
        sd = tmp_path / "secrets.d"
        sd.mkdir()
        (sd / "api.env").write_text("GCP_KEY=gcp-value\n")
        (sd / "db.env").write_text("DB_PASS=s3cret\n")
        provider = DotenvSecretProvider(secrets_dir=tmp_path)
        assert await provider.get_secret("GCP_KEY") == "gcp-value"
        assert await provider.get_secret("DB_PASS") == "s3cret"

    async def test_secrets_d_overrides_secrets_env(self, tmp_path: Path):
        (tmp_path / "secrets.env").write_text("KEY=base\n")
        sd = tmp_path / "secrets.d"
        sd.mkdir()
        (sd / "override.env").write_text("KEY=overridden\n")
        provider = DotenvSecretProvider(secrets_dir=tmp_path)
        assert await provider.get_secret("KEY") == "overridden"

    async def test_secrets_d_alphabetical_order(self, tmp_path: Path):
        (tmp_path / "secrets.env").write_text("")
        sd = tmp_path / "secrets.d"
        sd.mkdir()
        (sd / "a.env").write_text("KEY=from-a\n")
        (sd / "b.env").write_text("KEY=from-b\n")
        provider = DotenvSecretProvider(secrets_dir=tmp_path)
        # b.env loaded after a.env, so b wins
        assert await provider.get_secret("KEY") == "from-b"

    async def test_list_secrets_combined(self, tmp_path: Path):
        (tmp_path / "secrets.env").write_text("A=1\nB=2\n")
        sd = tmp_path / "secrets.d"
        sd.mkdir()
        (sd / "extra.env").write_text("C=3\n")
        provider = DotenvSecretProvider(secrets_dir=tmp_path)
        result = await provider.list_secrets()
        assert set(result) == {"A", "B", "C"}

    async def test_empty_secrets_dir(self, tmp_path: Path):
        provider = DotenvSecretProvider(secrets_dir=tmp_path)
        assert await provider.get_secret("X") is None
        assert await provider.list_secrets() == []

    async def test_caching_after_first_load(self, tmp_path: Path):
        (tmp_path / "secrets.env").write_text("K=original\n")
        provider = DotenvSecretProvider(secrets_dir=tmp_path)
        assert await provider.get_secret("K") == "original"
        # Modify file after first load
        (tmp_path / "secrets.env").write_text("K=modified\n")
        # Cached value returned
        assert await provider.get_secret("K") == "original"


@pytest.mark.asyncio
class TestSecretProviderFactoryIntegration:
    """Factory integration with real provider construction."""

    async def test_none_config_creates_dotenv(self, tmp_path: Path):
        (tmp_path / "secrets.env").write_text("KEY=val\n")
        provider = create_secret_provider(None, secrets_dir=tmp_path)
        assert isinstance(provider, DotenvSecretProvider)
        assert await provider.get_secret("KEY") == "val"

    async def test_explicit_dotenv_config(self, tmp_path: Path):
        (tmp_path / "secrets.env").write_text("KEY=val\n")
        config = SecretsConfig(provider=SecretsProviderType.DOTENV)
        provider = create_secret_provider(config, secrets_dir=tmp_path)
        assert await provider.get_secret("KEY") == "val"

    async def test_gcp_missing_project_id_raises(self):
        config = SecretsConfig(provider=SecretsProviderType.GCP, settings={})
        with pytest.raises(ValueError, match="project_id"):
            create_secret_provider(config)

    async def test_gcp_empty_project_id_raises(self):
        config = SecretsConfig(provider=SecretsProviderType.GCP, settings={"project_id": ""})
        with pytest.raises(ValueError, match="project_id"):
            create_secret_provider(config)
