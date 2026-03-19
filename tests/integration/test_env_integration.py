"""Integration tests for full env validation flow."""

from pathlib import Path

import pytest

from tanren_core.env import load_and_validate_env


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
