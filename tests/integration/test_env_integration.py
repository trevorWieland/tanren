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
