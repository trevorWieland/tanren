"""Tests for env validator module."""

import pytest

from tanren_core.env.schema import EnvBlock, OptionalEnvVar, RequiredEnvVar
from tanren_core.env.validator import VarStatus, validate_env


@pytest.mark.asyncio
class TestValidateRequired:
    async def test_pass(self):
        block = EnvBlock(required=[RequiredEnvVar(key="API_KEY")])
        merged = {"API_KEY": "sk-abc123"}
        source = {"API_KEY": ".env"}
        report = await validate_env(block, merged, source)
        assert report.passed
        assert len(report.required_results) == 1
        assert report.required_results[0].status == VarStatus.PASS

    async def test_missing(self):
        block = EnvBlock(required=[RequiredEnvVar(key="API_KEY", hint="get one")])
        report = await validate_env(block, {}, {})
        assert not report.passed
        assert report.required_results[0].status == VarStatus.MISSING
        assert report.required_results[0].hint == "get one"

    async def test_empty(self):
        block = EnvBlock(required=[RequiredEnvVar(key="API_KEY")])
        report = await validate_env(block, {"API_KEY": ""}, {"API_KEY": ".env"})
        assert not report.passed
        assert report.required_results[0].status == VarStatus.EMPTY

    async def test_pattern_match(self):
        block = EnvBlock(required=[RequiredEnvVar(key="K", pattern="^sk-or-v1-")])
        report = await validate_env(block, {"K": "sk-or-v1-abc123"}, {"K": ".env"})
        assert report.passed
        assert report.required_results[0].status == VarStatus.PASS

    async def test_pattern_mismatch(self):
        block = EnvBlock(required=[RequiredEnvVar(key="K", pattern="^sk-or-v1-")])
        report = await validate_env(block, {"K": "wrong-prefix"}, {"K": ".env"})
        assert not report.passed
        assert report.required_results[0].status == VarStatus.PATTERN_MISMATCH
        # Value should be redacted in message
        assert "wron..." in report.required_results[0].message

    async def test_multiple_required_partial_fail(self):
        block = EnvBlock(
            required=[
                RequiredEnvVar(key="A"),
                RequiredEnvVar(key="B"),
            ]
        )
        report = await validate_env(block, {"A": "val"}, {"A": ".env"})
        assert not report.passed
        statuses = {r.key: r.status for r in report.required_results}
        assert statuses["A"] == VarStatus.PASS
        assert statuses["B"] == VarStatus.MISSING

    async def test_all_pass(self):
        block = EnvBlock(
            required=[
                RequiredEnvVar(key="A"),
                RequiredEnvVar(key="B"),
            ]
        )
        merged = {"A": "1", "B": "2"}
        source = {"A": ".env", "B": ".env"}
        report = await validate_env(block, merged, source)
        assert report.passed

    async def test_from_os_environ(self, monkeypatch):
        monkeypatch.setenv("API_KEY", "sk-live-123")
        block = EnvBlock(required=[RequiredEnvVar(key="API_KEY")])
        report = await validate_env(block, {}, {})
        assert report.passed
        assert report.required_results[0].source == "os.environ"


@pytest.mark.asyncio
class TestValidateOptional:
    async def test_present(self):
        block = EnvBlock(optional=[OptionalEnvVar(key="LOG_LEVEL")])
        report = await validate_env(block, {"LOG_LEVEL": "DEBUG"}, {"LOG_LEVEL": ".env"})
        assert report.passed
        assert report.optional_results[0].status == VarStatus.PASS

    async def test_missing_with_default(self):
        block = EnvBlock(optional=[OptionalEnvVar(key="LOG_LEVEL", default="INFO")])
        merged: dict[str, str] = {}
        source: dict[str, str] = {}
        report = await validate_env(block, merged, source)
        assert report.passed
        assert report.optional_results[0].status == VarStatus.DEFAULTED
        # Default should be injected into merged env
        assert merged["LOG_LEVEL"] == "INFO"

    async def test_missing_no_default(self):
        block = EnvBlock(optional=[OptionalEnvVar(key="EXTRA")])
        report = await validate_env(block, {}, {})
        assert report.passed  # optional missing is not a failure
        assert report.optional_results[0].status == VarStatus.MISSING

    async def test_pattern_mismatch_warning(self):
        block = EnvBlock(optional=[OptionalEnvVar(key="URL", pattern="^https://")])
        report = await validate_env(block, {"URL": "http://bad"}, {"URL": ".env"})
        assert report.passed  # optional pattern mismatch is not a hard failure
        assert report.optional_results[0].status == VarStatus.PATTERN_MISMATCH
        assert len(report.warnings) == 1

    async def test_pattern_match(self):
        block = EnvBlock(optional=[OptionalEnvVar(key="URL", pattern="^https://")])
        report = await validate_env(block, {"URL": "https://ok"}, {"URL": ".env"})
        assert report.optional_results[0].status == VarStatus.PASS


@pytest.mark.asyncio
class TestRedaction:
    async def test_short_value_fully_redacted(self):
        block = EnvBlock(required=[RequiredEnvVar(key="K", pattern="^abc$")])
        report = await validate_env(block, {"K": "xy"}, {"K": ".env"})
        assert "****" in report.required_results[0].message

    async def test_long_value_partially_redacted(self):
        block = EnvBlock(required=[RequiredEnvVar(key="K", pattern="^abc$")])
        report = await validate_env(block, {"K": "xyzzzzz"}, {"K": ".env"})
        assert "xyzz..." in report.required_results[0].message


@pytest.mark.asyncio
class TestEmptyBlock:
    async def test_no_vars(self):
        block = EnvBlock()
        report = await validate_env(block, {}, {})
        assert report.passed
        assert report.required_results == []
        assert report.optional_results == []


@pytest.mark.asyncio
class TestSecretResolution:
    """Tests for source: 'secret:X' resolution via SecretProvider."""

    async def test_source_resolved_from_provider(self):
        """Provider returns value for source: secret:X -> PASS."""
        provider = _MockProvider({"my-api-key": "sk-resolved"})
        block = EnvBlock(required=[RequiredEnvVar(key="API_KEY", source="secret:my-api-key")])
        merged: dict[str, str] = {}
        source_map: dict[str, str] = {}
        report = await validate_env(block, merged, source_map, secret_provider=provider)
        assert report.passed
        assert merged["API_KEY"] == "sk-resolved"
        assert source_map["API_KEY"] == "secret:my-api-key"

    async def test_source_missing_from_provider(self):
        """Provider returns None, not in env -> MISSING."""
        provider = _MockProvider({})
        block = EnvBlock(required=[RequiredEnvVar(key="API_KEY", source="secret:missing")])
        report = await validate_env(block, {}, {}, secret_provider=provider)
        assert not report.passed
        assert report.required_results[0].status == VarStatus.MISSING

    async def test_env_overrides_provider(self):
        """Value already in merged_env -> provider not queried."""
        provider = _MockProvider({"my-key": "from-provider"})
        block = EnvBlock(required=[RequiredEnvVar(key="API_KEY", source="secret:my-key")])
        merged = {"API_KEY": "from-env"}
        source_map = {"API_KEY": ".env"}
        report = await validate_env(block, merged, source_map, secret_provider=provider)
        assert report.passed
        assert merged["API_KEY"] == "from-env"

    async def test_no_provider_ignores_source(self):
        """No provider passed -> source field ignored, falls back to env."""
        block = EnvBlock(required=[RequiredEnvVar(key="API_KEY", source="secret:my-key")])
        report = await validate_env(block, {}, {})
        assert not report.passed
        assert report.required_results[0].status == VarStatus.MISSING

    async def test_optional_source_resolved(self):
        """Optional var with source -> resolved from provider."""
        provider = _MockProvider({"opt-key": "opt-value"})
        block = EnvBlock(optional=[OptionalEnvVar(key="OPT", source="secret:opt-key")])
        merged: dict[str, str] = {}
        report = await validate_env(block, merged, {}, secret_provider=provider)
        assert report.passed
        assert merged["OPT"] == "opt-value"
        assert report.optional_results[0].status == VarStatus.PASS

    async def test_source_value_validated_against_pattern(self):
        """Resolved value fails pattern check -> PATTERN_MISMATCH."""
        provider = _MockProvider({"my-key": "wrong-prefix-value"})
        block = EnvBlock(required=[RequiredEnvVar(key="K", source="secret:my-key", pattern="^sk-")])
        report = await validate_env(block, {}, {}, secret_provider=provider)
        assert not report.passed
        assert report.required_results[0].status == VarStatus.PATTERN_MISMATCH

    async def test_os_environ_overrides_provider(self, monkeypatch):
        """os.environ takes priority over secret provider."""
        monkeypatch.setenv("API_KEY", "from-os-env")
        provider = _MockProvider({"my-key": "from-provider"})
        block = EnvBlock(required=[RequiredEnvVar(key="API_KEY", source="secret:my-key")])
        report = await validate_env(block, {}, {}, secret_provider=provider)
        assert report.passed
        assert report.required_results[0].source == "os.environ"


class _MockProvider:
    """Simple mock SecretProvider for testing."""

    def __init__(self, secrets: dict[str, str]) -> None:
        self._secrets = secrets

    async def get_secret(self, secret_id: str, *, version: str = "latest") -> str | None:
        return self._secrets.get(secret_id)

    async def list_secrets(self) -> list[str]:
        return list(self._secrets.keys())
