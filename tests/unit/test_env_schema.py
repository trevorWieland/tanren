"""Tests for env schema models."""

import pytest
from pydantic import ValidationError

from tanren_core.env.schema import (
    EnvBlock,
    OnMissing,
    OptionalEnvVar,
    RequiredEnvVar,
    SecretsConfig,
    SecretsProviderType,
    TanrenConfig,
)


class TestOnMissing:
    def test_values(self):
        assert OnMissing.ERROR == "error"
        assert OnMissing.WARN == "warn"
        assert OnMissing.PROMPT == "prompt"


class TestRequiredEnvVar:
    def test_minimal(self):
        v = RequiredEnvVar(key="MY_KEY")
        assert v.key == "MY_KEY"
        assert not v.description
        assert v.pattern is None
        assert not v.hint
        assert v.source is None

    def test_full(self):
        v = RequiredEnvVar(
            key="API_KEY",
            description="The API key",
            pattern="^sk-",
            hint="Get one at example.com",
        )
        assert v.pattern == "^sk-"
        assert v.hint == "Get one at example.com"

    def test_with_source(self):
        v = RequiredEnvVar(key="K", source="secret:my-api-key")
        assert v.source == "secret:my-api-key"


class TestOptionalEnvVar:
    def test_with_default(self):
        v = OptionalEnvVar(key="LOG_LEVEL", default="INFO")
        assert v.default == "INFO"

    def test_no_default(self):
        v = OptionalEnvVar(key="EXTRA")
        assert v.default is None
        assert v.source is None

    def test_with_source(self):
        v = OptionalEnvVar(key="K", source="secret:opt-key")
        assert v.source == "secret:opt-key"


class TestEnvBlock:
    def test_defaults(self):
        block = EnvBlock()
        assert block.on_missing == OnMissing.ERROR
        assert block.required == []
        assert block.optional == []

    def test_with_vars(self):
        block = EnvBlock(
            on_missing=OnMissing.WARN,
            required=[RequiredEnvVar(key="A")],
            optional=[OptionalEnvVar(key="B", default="x")],
        )
        assert block.on_missing == OnMissing.WARN
        assert len(block.required) == 1
        assert len(block.optional) == 1


class TestSecretsConfig:
    def test_defaults(self):
        c = SecretsConfig()
        assert c.provider == SecretsProviderType.DOTENV
        assert c.settings == {}

    def test_gcp_provider(self):
        c = SecretsConfig(provider=SecretsProviderType.GCP, settings={"project_id": "my-proj"})
        assert c.provider == SecretsProviderType.GCP
        assert c.settings["project_id"] == "my-proj"

    def test_unknown_provider_rejected(self):
        with pytest.raises(ValidationError):
            SecretsConfig.model_validate({"provider": "aws"})


class TestTanrenConfig:
    def test_without_env(self):
        c = TanrenConfig(version="0.1.0", profile="python-uv", installed="2026-03-07")
        assert c.env is None
        assert c.secrets is None

    def test_with_env(self):
        c = TanrenConfig(
            version="0.1.0",
            profile="python-uv",
            installed="2026-03-07",
            env=EnvBlock(required=[RequiredEnvVar(key="K")]),
        )
        assert c.env is not None
        assert len(c.env.required) == 1

    def test_with_secrets(self):
        c = TanrenConfig(
            version="0.1.0",
            profile="python-uv",
            installed="2026-03-07",
            secrets=SecretsConfig(
                provider=SecretsProviderType.GCP,
                settings={"project_id": "test"},
            ),
        )
        assert c.secrets is not None
        assert c.secrets.provider == SecretsProviderType.GCP

    def test_secrets_defaults_to_none(self):
        c = TanrenConfig(version="0.1.0", profile="python-uv", installed="2026-03-07")
        assert c.secrets is None
