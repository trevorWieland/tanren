"""Tests for env secrets module."""

import stat
from pathlib import Path

from tanren_core.env.secrets import ensure_secrets_dir, list_secrets, redact, set_secret


class TestRedact:
    def test_long_value(self):
        assert redact("sk-or-v1-abc123") == "sk-o..."

    def test_short_value(self):
        assert redact("abc") == "****"

    def test_exactly_six(self):
        assert redact("abcdef") == "abcd..."

    def test_five_chars(self):
        assert redact("abcde") == "****"


class TestEnsureSecretsDir:
    def test_creates_dir(self, tmp_path: Path):
        d = tmp_path / "secrets"
        result = ensure_secrets_dir(d)
        assert result == d
        assert d.is_dir()
        mode = stat.S_IMODE(d.stat().st_mode)
        assert mode == 0o700

    def test_idempotent(self, tmp_path: Path):
        d = tmp_path / "secrets"
        ensure_secrets_dir(d)
        ensure_secrets_dir(d)  # should not raise
        assert d.is_dir()


class TestSetSecret:
    def test_creates_file(self, tmp_path: Path):
        secrets = tmp_path / "secrets"
        path = set_secret("MY_KEY", "my_value", secrets_dir=secrets)
        assert path.exists()
        content = path.read_text()
        assert "MY_KEY" in content
        assert "my_value" in content
        mode = stat.S_IMODE(path.stat().st_mode)
        assert mode == 0o600

    def test_update_existing(self, tmp_path: Path):
        secrets = tmp_path / "secrets"
        set_secret("MY_KEY", "old_value", secrets_dir=secrets)
        set_secret("MY_KEY", "new_value", secrets_dir=secrets)
        path = secrets / "secrets.env"
        content = path.read_text()
        assert "new_value" in content
        assert "old_value" not in content

    def test_multiple_keys(self, tmp_path: Path):
        secrets = tmp_path / "secrets"
        set_secret("KEY_A", "val_a", secrets_dir=secrets)
        set_secret("KEY_B", "val_b", secrets_dir=secrets)
        path = secrets / "secrets.env"
        content = path.read_text()
        assert "KEY_A" in content
        assert "KEY_B" in content

    def test_secrets_dir_permissions(self, tmp_path: Path):
        secrets = tmp_path / "secrets"
        set_secret("K", "V", secrets_dir=secrets)
        mode = stat.S_IMODE(secrets.stat().st_mode)
        assert mode == 0o700


class TestListSecrets:
    def test_empty(self, tmp_path: Path):
        result = list_secrets(secrets_dir=tmp_path / "noexist")
        assert result == []

    def test_lists_with_redaction(self, tmp_path: Path):
        secrets = tmp_path / "secrets"
        set_secret("API_KEY", "sk-or-v1-abc123", secrets_dir=secrets)
        result = list_secrets(secrets_dir=secrets)
        assert len(result) == 1
        key, redacted = result[0]
        assert key == "API_KEY"
        assert redacted == "sk-o..."
        assert "abc123" not in redacted
