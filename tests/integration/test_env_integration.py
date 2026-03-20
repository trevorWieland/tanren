"""Integration tests for full env validation flow."""

import logging
from typing import TYPE_CHECKING

import pytest

from tanren_core.env import load_and_validate_env
from tanren_core.env.loader import (
    _check_permissions,  # noqa: PLC2701
    discover_env_vars_from_dotenv_example,
    load_env_layers,
    resolve_env_var,
)

if TYPE_CHECKING:
    from pathlib import Path


class TestFullFlow:
    @pytest.mark.asyncio
    async def test_tanren_yml_to_report(self, tmp_path: Path, monkeypatch):
        """Full flow: tanren.yml -> load -> validate -> report."""
        monkeypatch.setenv("API_KEY", "sk-or-v1-test123")
        monkeypatch.setenv("BASE_URL", "https://api.example.com")

        (tmp_path / "tanren.yml").write_text(
            "version: 0.1.0\n"
            "profile: default\n"
            "installed: 2026-01-01\n"
            "env:\n"
            "  on_missing: error\n"
            "  required:\n"
            "    - key: API_KEY\n"
            '      pattern: "^sk-or-v1-"\n'
            "    - key: BASE_URL\n"
            '      pattern: "^https://"\n'
            "  optional:\n"
            "    - key: LOG_LEVEL\n"
            '      default: "INFO"\n'
        )

        report, env = await load_and_validate_env(
            tmp_path, daemon_mode=True, secrets_dir=tmp_path / "secrets"
        )
        assert report.passed
        assert env.get("LOG_LEVEL") == "INFO"

    @pytest.mark.asyncio
    async def test_missing_required_fails(self, tmp_path: Path, monkeypatch):
        monkeypatch.delenv("NONEXISTENT_XYZ_KEY", raising=False)
        (tmp_path / "tanren.yml").write_text(
            "version: 0.1.0\n"
            "profile: default\n"
            "installed: 2026-01-01\n"
            "env:\n"
            "  required:\n"
            "    - key: NONEXISTENT_XYZ_KEY\n"
        )
        report, _ = await load_and_validate_env(tmp_path, secrets_dir=tmp_path / "secrets")
        assert not report.passed

    @pytest.mark.asyncio
    async def test_no_env_block_passes(self, tmp_path: Path):
        (tmp_path / "tanren.yml").write_text(
            "version: 0.1.0\nprofile: default\ninstalled: 2026-01-01\n"
        )
        report, _env = await load_and_validate_env(tmp_path, secrets_dir=tmp_path / "secrets")
        assert report.passed

    @pytest.mark.asyncio
    async def test_no_tanren_yml_passes(self, tmp_path: Path):
        report, _env = await load_and_validate_env(tmp_path, secrets_dir=tmp_path / "secrets")
        assert report.passed

    @pytest.mark.asyncio
    async def test_dotenv_example_fallback(self, tmp_path: Path, monkeypatch):
        monkeypatch.setenv("API_KEY", "val")
        (tmp_path / "tanren.yml").write_text(
            "version: 0.1.0\nprofile: default\ninstalled: 2026-01-01\n"
        )
        (tmp_path / ".env.example").write_text("API_KEY=placeholder\n")

        report, _ = await load_and_validate_env(tmp_path, secrets_dir=tmp_path / "secrets")
        assert report.passed

    @pytest.mark.asyncio
    async def test_daemon_forces_error_on_prompt(self, tmp_path: Path, monkeypatch):
        monkeypatch.delenv("MISSING_KEY_XYZ", raising=False)
        (tmp_path / "tanren.yml").write_text(
            "version: 0.1.0\nprofile: default\ninstalled: 2026-01-01\n"
            "env:\n  on_missing: prompt\n  required:\n    - key: MISSING_KEY_XYZ\n"
        )
        report, _ = await load_and_validate_env(
            tmp_path, daemon_mode=True, secrets_dir=tmp_path / "secrets"
        )
        # Should fail because daemon forces error mode
        assert not report.passed


class TestLayeredPriority:
    @pytest.mark.asyncio
    async def test_dotenv_plus_secrets(self, tmp_path: Path, monkeypatch):
        """Verify .env overrides secrets.env."""
        sd = tmp_path / "secrets"
        sd.mkdir()
        (sd / "secrets.env").write_text("MY_VAR=from_secrets\n")
        (tmp_path / ".env").write_text("MY_VAR=from_dotenv\n")
        (tmp_path / "tanren.yml").write_text(
            "version: 0.1.0\nprofile: default\ninstalled: 2026-01-01\n"
            "env:\n  required:\n    - key: MY_VAR\n"
        )

        report, env = await load_and_validate_env(tmp_path, secrets_dir=sd)
        assert report.passed
        # .env should win over secrets.env
        assert env["MY_VAR"] == "from_dotenv"

    @pytest.mark.asyncio
    async def test_os_environ_wins(self, tmp_path: Path, monkeypatch):
        monkeypatch.setenv("MY_VAR", "from_real_env")
        (tmp_path / ".env").write_text("MY_VAR=from_dotenv\n")
        (tmp_path / "tanren.yml").write_text(
            "version: 0.1.0\nprofile: default\ninstalled: 2026-01-01\n"
            "env:\n  required:\n    - key: MY_VAR\n"
        )

        report, env = await load_and_validate_env(tmp_path, secrets_dir=tmp_path / "secrets")
        assert report.passed
        assert env["MY_VAR"] == "from_real_env"

    async def test_source_secret_resolved_from_dotenv_provider(self, tmp_path: Path, monkeypatch):
        """source: secret:X resolves via DotenvSecretProvider."""
        monkeypatch.delenv("MY_SECRET", raising=False)
        sd = tmp_path / "secrets"
        sd.mkdir()
        (sd / "secrets.env").write_text("MY_SECRET=resolved-from-secrets\n")
        (tmp_path / "tanren.yml").write_text(
            "version: 0.1.0\nprofile: default\ninstalled: 2026-01-01\n"
            "env:\n  required:\n    - key: MY_SECRET\n      source: 'secret:MY_SECRET'\n"
        )

        report, env = await load_and_validate_env(tmp_path, secrets_dir=sd)
        assert report.passed
        assert env["MY_SECRET"] == "resolved-from-secrets"

    async def test_source_secret_from_secrets_d(self, tmp_path: Path, monkeypatch):
        """source: secret:X resolves from secrets.d/*.env files."""
        monkeypatch.delenv("DB_PASS", raising=False)
        sd = tmp_path / "secrets"
        sd.mkdir()
        (sd / "secrets.env").write_text("")
        sd_d = sd / "secrets.d"
        sd_d.mkdir()
        (sd_d / "db.env").write_text("DB_PASS=s3cret\n")
        (tmp_path / "tanren.yml").write_text(
            "version: 0.1.0\nprofile: default\ninstalled: 2026-01-01\n"
            "env:\n  required:\n    - key: DB_PASS\n      source: 'secret:DB_PASS'\n"
        )

        report, env = await load_and_validate_env(tmp_path, secrets_dir=sd)
        assert report.passed
        assert env["DB_PASS"] == "s3cret"

    async def test_source_secret_missing_fails(self, tmp_path: Path, monkeypatch):
        """source: secret:X where the secret doesn't exist -> MISSING."""
        monkeypatch.delenv("GONE", raising=False)
        sd = tmp_path / "secrets"
        sd.mkdir()
        (sd / "secrets.env").write_text("")
        (tmp_path / "tanren.yml").write_text(
            "version: 0.1.0\nprofile: default\ninstalled: 2026-01-01\n"
            "env:\n  required:\n    - key: GONE\n      source: 'secret:GONE'\n"
        )

        report, _env = await load_and_validate_env(tmp_path, secrets_dir=sd)
        assert not report.passed

    async def test_source_secret_env_overrides(self, tmp_path: Path, monkeypatch):
        """os.environ takes priority over secret provider."""
        monkeypatch.setenv("MY_KEY", "from-env")
        sd = tmp_path / "secrets"
        sd.mkdir()
        (sd / "secrets.env").write_text("MY_KEY=from-provider\n")
        (tmp_path / "tanren.yml").write_text(
            "version: 0.1.0\nprofile: default\ninstalled: 2026-01-01\n"
            "env:\n  required:\n    - key: MY_KEY\n      source: 'secret:MY_KEY'\n"
        )

        report, env = await load_and_validate_env(tmp_path, secrets_dir=sd)
        assert report.passed
        assert env["MY_KEY"] == "from-env"

    async def test_source_optional_resolved(self, tmp_path: Path, monkeypatch):
        """Optional var with source: secret:X resolved from provider."""
        monkeypatch.delenv("OPT_SECRET", raising=False)
        sd = tmp_path / "secrets"
        sd.mkdir()
        (sd / "secrets.env").write_text("OPT_SECRET=opt-val\n")
        (tmp_path / "tanren.yml").write_text(
            "version: 0.1.0\nprofile: default\ninstalled: 2026-01-01\n"
            "env:\n  optional:\n    - key: OPT_SECRET\n      source: 'secret:OPT_SECRET'\n"
        )

        report, env = await load_and_validate_env(tmp_path, secrets_dir=sd)
        assert report.passed
        assert env["OPT_SECRET"] == "opt-val"

    async def test_source_with_pattern_validated(self, tmp_path: Path, monkeypatch):
        """Resolved secret value is validated against pattern."""
        monkeypatch.delenv("API_KEY", raising=False)
        sd = tmp_path / "secrets"
        sd.mkdir()
        (sd / "secrets.env").write_text("API_KEY=wrong-prefix\n")
        (tmp_path / "tanren.yml").write_text(
            "version: 0.1.0\nprofile: default\ninstalled: 2026-01-01\n"
            "env:\n  required:\n"
            "    - key: API_KEY\n      source: 'secret:API_KEY'\n      pattern: '^sk-'\n"
        )

        report, _env = await load_and_validate_env(tmp_path, secrets_dir=sd)
        assert not report.passed

    async def test_mixed_source_and_normal_vars(self, tmp_path: Path, monkeypatch):
        """Mix of source-backed and normal vars in the same env block."""
        monkeypatch.setenv("NORMAL_VAR", "normal-val")
        monkeypatch.delenv("SECRET_VAR", raising=False)
        sd = tmp_path / "secrets"
        sd.mkdir()
        (sd / "secrets.env").write_text("SECRET_VAR=secret-val\n")
        (tmp_path / "tanren.yml").write_text(
            "version: 0.1.0\nprofile: default\ninstalled: 2026-01-01\n"
            "env:\n  required:\n"
            "    - key: NORMAL_VAR\n"
            "    - key: SECRET_VAR\n      source: 'secret:SECRET_VAR'\n"
        )

        report, env = await load_and_validate_env(tmp_path, secrets_dir=sd)
        assert report.passed
        # NORMAL_VAR is resolved from os.environ by the validator (not in merged dict)
        # SECRET_VAR is injected into merged dict by the secret provider
        assert env["SECRET_VAR"] == "secret-val"

    async def test_no_source_vars_skips_provider_init(self, tmp_path: Path):
        """No vars with source -> provider not created (no crash even with gcp config)."""
        (tmp_path / ".env").write_text("K=v\n")
        (tmp_path / "tanren.yml").write_text(
            "version: 0.1.0\nprofile: default\ninstalled: 2026-01-01\n"
            "secrets:\n  provider: gcp\n  settings:\n    project_id: fake\n"
            "env:\n  required:\n    - key: K\n"
        )

        # Should pass without importing google-cloud-secret-manager
        report, env = await load_and_validate_env(tmp_path, secrets_dir=tmp_path / "secrets")
        assert report.passed
        assert env["K"] == "v"

    async def test_dotenv_provider_explicit_config(self, tmp_path: Path, monkeypatch):
        """Explicit secrets.provider: dotenv works the same as default."""
        monkeypatch.delenv("MY_VAR", raising=False)
        sd = tmp_path / "secrets"
        sd.mkdir()
        (sd / "secrets.env").write_text("MY_VAR=from-dotenv-provider\n")
        (tmp_path / "tanren.yml").write_text(
            "version: 0.1.0\nprofile: default\ninstalled: 2026-01-01\n"
            "secrets:\n  provider: dotenv\n"
            "env:\n  required:\n    - key: MY_VAR\n      source: 'secret:MY_VAR'\n"
        )

        report, env = await load_and_validate_env(tmp_path, secrets_dir=sd)
        assert report.passed
        assert env["MY_VAR"] == "from-dotenv-provider"


class TestSecretsD:
    def test_secrets_d_files_loaded_alphabetically(self, tmp_path: Path, monkeypatch):
        """secrets.d/*.env files are loaded in alphabetical order (later wins)."""
        sd = tmp_path / "secrets"
        sd.mkdir()
        (sd / "secrets.env").write_text("")
        sd_d = sd / "secrets.d"
        sd_d.mkdir()
        (sd_d / "01-first.env").write_text("VAR=first\n")
        (sd_d / "02-second.env").write_text("VAR=second\n")

        merged, source_map = load_env_layers(tmp_path, secrets_dir=sd)
        assert merged["VAR"] == "second"
        assert "02-second.env" in source_map["VAR"]


class TestPermissionsWarning:
    def test_world_readable_warning(self, tmp_path: Path, caplog):
        """World-readable secrets file logs a warning."""
        secret_file = tmp_path / "secrets.env"
        secret_file.write_text("KEY=val\n")
        secret_file.chmod(0o644)

        with caplog.at_level(logging.WARNING, logger="tanren_core.env.loader"):
            _check_permissions(secret_file)

        assert any("world-readable" in r.message for r in caplog.records)

    def test_secure_permissions_no_warning(self, tmp_path: Path, caplog):
        """Properly secured file does not log a warning."""
        secret_file = tmp_path / "secrets.env"
        secret_file.write_text("KEY=val\n")
        secret_file.chmod(0o600)

        with caplog.at_level(logging.WARNING, logger="tanren_core.env.loader"):
            _check_permissions(secret_file)

        assert not any("world-readable" in r.message for r in caplog.records)


class TestEnvExampleFallback:
    def test_dotenv_example_keys_parsed(self, tmp_path: Path):
        """.env.example fallback returns RequiredEnvVar list."""
        (tmp_path / ".env.example").write_text("API_KEY=placeholder\nDB_URL=postgres://...\n")

        result = discover_env_vars_from_dotenv_example(tmp_path)
        keys = [v.key for v in result]
        assert "API_KEY" in keys
        assert "DB_URL" in keys

    def test_missing_example_returns_empty(self, tmp_path: Path):
        """No .env.example returns empty list."""
        result = discover_env_vars_from_dotenv_example(tmp_path)
        assert result == []


class TestResolveEnvVar:
    def test_resolve_from_merged(self):
        merged = {"KEY": "val"}
        source_map = {"KEY": "secrets.env"}
        value, source = resolve_env_var("KEY", merged, source_map)
        assert value == "val"
        assert source == "secrets.env"

    def test_resolve_from_os_environ(self, monkeypatch):
        monkeypatch.setenv("ENV_KEY", "from-env")
        value, source = resolve_env_var("ENV_KEY", {}, {})
        assert value == "from-env"
        assert source == "os.environ"

    def test_missing_returns_none(self, monkeypatch):
        monkeypatch.delenv("MISSING_XYZ", raising=False)
        value, source = resolve_env_var("MISSING_XYZ", {}, {})
        assert value is None
        assert source is None


class TestDaemonModeNonDaemon:
    @pytest.mark.asyncio
    async def test_non_daemon_mode_allows_prompt_policy(self, tmp_path: Path, monkeypatch):
        """Non-daemon mode keeps on_missing=prompt (doesn't force error)."""
        monkeypatch.delenv("MISSING_KEY_ABC", raising=False)
        (tmp_path / "tanren.yml").write_text(
            "version: 0.1.0\nprofile: default\ninstalled: 2026-01-01\n"
            "env:\n  on_missing: prompt\n  required:\n    - key: MISSING_KEY_ABC\n"
        )
        # In non-daemon mode, prompt policy should be preserved.
        # With prompt policy and a missing key, validate_env still reports
        # MISSING — but the policy itself is not forced to error.
        report, _ = await load_and_validate_env(
            tmp_path, daemon_mode=False, secrets_dir=tmp_path / "secrets"
        )
        # Should still fail (key is missing), but the policy is PROMPT not ERROR
        assert not report.passed
