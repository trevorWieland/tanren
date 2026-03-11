"""Tests for env loader module."""

from pathlib import Path

import pytest
from pydantic import ValidationError

from worker_manager.env.loader import (
    discover_env_vars_from_dotenv_example,
    load_env_layers,
    parse_tanren_yml,
    resolve_env_var,
)


class TestParseTanrenYml:
    def test_valid_with_environment_block(self, tmp_path: Path):
        yml = tmp_path / "tanren.yml"
        yml.write_text(
            "version: 0.1.0\n"
            "profile: default\n"
            "installed: 2026-01-01\n"
            "environment:\n"
            "  ci:\n"
            "    type: docker\n"
            "    gate_cmd: make test\n"
        )
        config = parse_tanren_yml(tmp_path)
        assert config is not None
        assert config.environment is not None
        assert config.environment["ci"] == {"type": "docker", "gate_cmd": "make test"}

    def test_unknown_top_level_key_rejected(self, tmp_path: Path):
        yml = tmp_path / "tanren.yml"
        yml.write_text(
            "version: 0.1.0\nprofile: default\ninstalled: 2026-01-01\nunknown_top_level_key: true\n"
        )
        with pytest.raises(ValidationError, match="Extra inputs are not permitted"):
            parse_tanren_yml(tmp_path)

    def test_valid_with_env(self, tmp_path: Path):
        yml = tmp_path / "tanren.yml"
        yml.write_text(
            "version: 0.1.0\n"
            "profile: python-uv\n"
            "installed: 2026-03-07\n"
            "env:\n"
            "  on_missing: error\n"
            "  required:\n"
            "    - key: API_KEY\n"
            '      description: "The key"\n'
            '      pattern: "^sk-"\n'
        )
        config = parse_tanren_yml(tmp_path)
        assert config is not None
        assert config.env is not None
        assert len(config.env.required) == 1
        assert config.env.required[0].key == "API_KEY"
        assert config.env.required[0].pattern == "^sk-"

    def test_valid_without_env(self, tmp_path: Path):
        yml = tmp_path / "tanren.yml"
        yml.write_text("version: 0.1.0\nprofile: default\ninstalled: 2026-01-01\n")
        config = parse_tanren_yml(tmp_path)
        assert config is not None
        assert config.env is None

    def test_missing_file(self, tmp_path: Path):
        config = parse_tanren_yml(tmp_path)
        assert config is None

    def test_empty_file(self, tmp_path: Path):
        yml = tmp_path / "tanren.yml"
        yml.write_text("")
        config = parse_tanren_yml(tmp_path)
        assert config is None

    def test_optional_vars(self, tmp_path: Path):
        yml = tmp_path / "tanren.yml"
        yml.write_text(
            "version: 0.1.0\n"
            "profile: default\n"
            "installed: 2026-01-01\n"
            "env:\n"
            "  optional:\n"
            "    - key: LOG_LEVEL\n"
            '      default: "INFO"\n'
        )
        config = parse_tanren_yml(tmp_path)
        assert config.env is not None
        assert len(config.env.optional) == 1
        assert config.env.optional[0].default == "INFO"


class TestDiscoverEnvVarsFromDotenvExample:
    def test_parses_keys(self, tmp_path: Path):
        example = tmp_path / ".env.example"
        example.write_text("API_KEY=your-key-here\nDB_URL=postgres://...\n")
        vars = discover_env_vars_from_dotenv_example(tmp_path)
        assert len(vars) == 2
        keys = {v.key for v in vars}
        assert keys == {"API_KEY", "DB_URL"}

    def test_no_file(self, tmp_path: Path):
        vars = discover_env_vars_from_dotenv_example(tmp_path)
        assert vars == []

    def test_empty_file(self, tmp_path: Path):
        example = tmp_path / ".env.example"
        example.write_text("")
        vars = discover_env_vars_from_dotenv_example(tmp_path)
        assert vars == []


class TestLoadEnvLayers:
    def test_dotenv_loaded(self, tmp_path: Path):
        dotenv = tmp_path / ".env"
        dotenv.write_text("MY_VAR=from_dotenv\n")
        merged, source_map = load_env_layers(tmp_path, secrets_dir=tmp_path / "secrets")
        assert merged["MY_VAR"] == "from_dotenv"
        assert source_map["MY_VAR"] == ".env"

    def test_secrets_env_loaded(self, tmp_path: Path):
        sd = tmp_path / "secrets"
        sd.mkdir()
        secrets = sd / "secrets.env"
        secrets.write_text("SECRET_KEY=abc123\n")
        merged, source_map = load_env_layers(tmp_path, secrets_dir=sd)
        assert merged["SECRET_KEY"] == "abc123"
        assert source_map["SECRET_KEY"] == str(secrets)

    def test_dotenv_overrides_secrets(self, tmp_path: Path):
        sd = tmp_path / "secrets"
        sd.mkdir()
        secrets = sd / "secrets.env"
        secrets.write_text("MY_VAR=from_secrets\n")
        dotenv = tmp_path / ".env"
        dotenv.write_text("MY_VAR=from_dotenv\n")
        merged, _ = load_env_layers(tmp_path, secrets_dir=sd)
        assert merged["MY_VAR"] == "from_dotenv"

    def test_os_environ_overrides_all(self, tmp_path: Path, monkeypatch):
        dotenv = tmp_path / ".env"
        dotenv.write_text("MY_VAR=from_dotenv\n")
        monkeypatch.setenv("MY_VAR", "from_env")
        merged, source_map = load_env_layers(tmp_path, secrets_dir=tmp_path / "secrets")
        assert merged["MY_VAR"] == "from_env"
        assert source_map["MY_VAR"] == "os.environ"

    def test_secrets_d_alphabetical(self, tmp_path: Path):
        sd = tmp_path / "secrets"
        secrets_d = sd / "secrets.d"
        secrets_d.mkdir(parents=True)
        (secrets_d / "a.env").write_text("VAR=from_a\n")
        (secrets_d / "b.env").write_text("VAR=from_b\n")
        merged, _ = load_env_layers(tmp_path, secrets_dir=sd)
        # b.env comes after a.env alphabetically, so it wins
        assert merged["VAR"] == "from_b"

    def test_world_readable_warning(self, tmp_path: Path, caplog):
        import os

        sd = tmp_path / "secrets"
        sd.mkdir()
        secrets = sd / "secrets.env"
        secrets.write_text("KEY=val\n")
        os.chmod(secrets, 0o644)
        import logging

        with caplog.at_level(logging.WARNING):
            load_env_layers(tmp_path, secrets_dir=sd)
        assert "world-readable" in caplog.text

    def test_no_sources(self, tmp_path: Path):
        merged, source_map = load_env_layers(tmp_path, secrets_dir=tmp_path / "secrets")
        assert merged == {}
        assert source_map == {}


class TestResolveEnvVar:
    def test_from_merged(self):
        merged = {"KEY": "val"}
        source = {"KEY": ".env"}
        val, src = resolve_env_var("KEY", merged, source)
        assert val == "val"
        assert src == ".env"

    def test_from_os_environ(self, monkeypatch):
        monkeypatch.setenv("KEY", "from_env")
        val, src = resolve_env_var("KEY", {}, {})
        assert val == "from_env"
        assert src == "os.environ"

    def test_missing(self):
        val, src = resolve_env_var("NOPE", {}, {})
        assert val is None
        assert src is None
