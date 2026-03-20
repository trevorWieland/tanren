"""Tests for secret provider factory."""

from typing import TYPE_CHECKING

import pytest

from tanren_core.adapters.dotenv_secret_provider import DotenvSecretProvider
from tanren_core.env.schema import SecretsConfig, SecretsProviderType
from tanren_core.env.secret_provider_factory import create_secret_provider

if TYPE_CHECKING:
    from pathlib import Path


class TestCreateSecretProvider:
    def test_none_config_returns_dotenv(self, tmp_path: Path):
        provider = create_secret_provider(None, secrets_dir=tmp_path)
        assert isinstance(provider, DotenvSecretProvider)

    def test_dotenv_explicit(self, tmp_path: Path):
        config = SecretsConfig(provider=SecretsProviderType.DOTENV)
        provider = create_secret_provider(config, secrets_dir=tmp_path)
        assert isinstance(provider, DotenvSecretProvider)

    def test_gcp_missing_project_id_raises(self):
        config = SecretsConfig(provider=SecretsProviderType.GCP, settings={})
        with pytest.raises(ValueError, match="project_id"):
            create_secret_provider(config)

    def test_gcp_empty_project_id_raises(self):
        config = SecretsConfig(provider=SecretsProviderType.GCP, settings={"project_id": ""})
        with pytest.raises(ValueError, match="project_id"):
            create_secret_provider(config)
