"""Integration tests for env/secrets.py and env/reporter.py utilities."""

import json
import sys
from typing import TYPE_CHECKING
from unittest.mock import patch

from tanren_core.env.reporter import (
    _format_var,  # noqa: PLC2701 — testing private implementation
    _icon,  # noqa: PLC2701 — testing private implementation
    format_report,
    format_report_json,
    supports_color,
)
from tanren_core.env.secrets import (
    ensure_secrets_dir,
    list_secrets,
    redact,
    set_secret,
)
from tanren_core.env.validator import EnvReport, VarResult, VarStatus

if TYPE_CHECKING:
    from pathlib import Path

    import pytest

# ---------------------------------------------------------------------------
# secrets.py tests
# ---------------------------------------------------------------------------


class TestRedact:
    def test_redact_short_value(self) -> None:
        assert redact("abc") == "****"

    def test_redact_long_value(self) -> None:
        assert redact("sk-or-v1-abc123") == "sk-o..."


class TestEnsureSecretsDir:
    def test_ensure_secrets_dir_creates_with_permissions(self, tmp_path: Path) -> None:
        target = tmp_path / "secrets"
        result = ensure_secrets_dir(target)
        assert result == target
        assert target.is_dir()
        mode = target.stat().st_mode & 0o777
        assert mode == 0o700


class TestSetSecret:
    def test_set_secret_creates_file(self, tmp_path: Path) -> None:
        set_secret("API_KEY", "val", tmp_path)
        secrets_file = tmp_path / "secrets.env"
        assert secrets_file.exists()
        mode = secrets_file.stat().st_mode & 0o777
        assert mode == 0o600
        content = secrets_file.read_text()
        assert "API_KEY" in content

    def test_set_secret_updates_existing(self, tmp_path: Path) -> None:
        set_secret("KEY", "old_value", tmp_path)
        set_secret("KEY", "new_value", tmp_path)
        content = (tmp_path / "secrets.env").read_text()
        assert "new_value" in content
        assert "old_value" not in content

    def test_set_secret_preserves_other_keys(self, tmp_path: Path) -> None:
        set_secret("KEY1", "value1", tmp_path)
        set_secret("KEY2", "value2", tmp_path)
        content = (tmp_path / "secrets.env").read_text()
        assert "KEY1" in content
        assert "KEY2" in content


class TestListSecrets:
    def test_list_secrets_returns_redacted(self, tmp_path: Path) -> None:
        set_secret("MY_SECRET", "super-secret-value", tmp_path)
        result = list_secrets(tmp_path)
        assert len(result) == 1
        key, redacted_val = result[0]
        assert key == "MY_SECRET"
        assert redacted_val == "supe..."

    def test_list_secrets_empty(self, tmp_path: Path) -> None:
        result = list_secrets(tmp_path)
        assert result == []


# ---------------------------------------------------------------------------
# reporter.py tests
# ---------------------------------------------------------------------------


class TestSupportsColor:
    def test_supports_color_no_color_env(self, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.setenv("NO_COLOR", "1")
        assert supports_color() is False

    def test_supports_color_without_no_color(self, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.delenv("NO_COLOR", raising=False)
        with patch.object(sys.stderr, "isatty", return_value=True):
            assert supports_color() is True

    def test_supports_color_not_tty(self, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.delenv("NO_COLOR", raising=False)
        with patch.object(sys.stderr, "isatty", return_value=False):
            assert supports_color() is False


class TestIcon:
    def test_icon_pass_no_color(self) -> None:
        assert _icon(VarStatus.PASS, color=False) == "[OK]"

    def test_icon_missing_no_color(self) -> None:
        assert _icon(VarStatus.MISSING, color=False) == "[FAIL]"

    def test_icon_defaulted_no_color(self) -> None:
        assert _icon(VarStatus.DEFAULTED, color=False) == "[DEFAULT]"

    def test_icon_pass_color(self) -> None:
        result = _icon(VarStatus.PASS, color=True)
        assert "\033[32m" in result

    def test_icon_missing_color(self) -> None:
        result = _icon(VarStatus.MISSING, color=True)
        assert "\033[31m" in result


class TestFormatVar:
    def test_format_var_pass_not_verbose_returns_none(self) -> None:
        var = VarResult(key="X", status=VarStatus.PASS)
        assert _format_var(var, color=False, verbose=False) is None

    def test_format_var_pass_verbose_returns_line(self) -> None:
        var = VarResult(key="X", status=VarStatus.PASS)
        result = _format_var(var, color=False, verbose=True)
        assert result is not None
        assert "X" in result

    def test_format_var_missing_with_hint(self) -> None:
        var = VarResult(key="TOKEN", status=VarStatus.MISSING, hint="set it")
        result = _format_var(var, color=False, verbose=False)
        assert result is not None
        assert "hint:" in result

    def test_format_var_with_source(self) -> None:
        var = VarResult(key="DB_URL", status=VarStatus.PASS, source="env")
        result = _format_var(var, color=False, verbose=True)
        assert result is not None
        assert "(from env)" in result

    def test_format_var_with_description_on_failure(self) -> None:
        var = VarResult(key="API_KEY", status=VarStatus.MISSING, description="API key")
        result = _format_var(var, color=False, verbose=False)
        assert result is not None
        assert "API key" in result


class TestFormatReport:
    @staticmethod
    def _pass_var(key: str = "GOOD") -> VarResult:
        return VarResult(key=key, status=VarStatus.PASS)

    @staticmethod
    def _missing_var(key: str = "BAD") -> VarResult:
        return VarResult(key=key, status=VarStatus.MISSING)

    @staticmethod
    def _defaulted_var(key: str = "OPT") -> VarResult:
        return VarResult(key=key, status=VarStatus.DEFAULTED)

    def test_format_report_passed(self, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.setenv("NO_COLOR", "1")
        report = EnvReport(
            passed=True,
            required_results=[self._pass_var()],
            optional_results=[],
            warnings=[],
        )
        output = format_report(report)
        assert "PASSED" in output
        assert "1 passed" in output

    def test_format_report_failed(self, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.setenv("NO_COLOR", "1")
        report = EnvReport(
            passed=False,
            required_results=[self._missing_var()],
            optional_results=[],
            warnings=[],
        )
        output = format_report(report)
        assert "FAILED" in output

    def test_format_report_verbose(self, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.setenv("NO_COLOR", "1")
        report = EnvReport(
            passed=True,
            required_results=[self._pass_var("SHOWN")],
            optional_results=[],
            warnings=[],
        )
        output = format_report(report, verbose=True)
        assert "SHOWN" in output

    def test_format_report_with_project_name(self, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.setenv("NO_COLOR", "1")
        report = EnvReport(
            passed=True,
            required_results=[],
            optional_results=[],
            warnings=[],
        )
        output = format_report(report, project_name="myapp")
        assert "myapp" in output

    def test_format_report_with_warnings(self, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.setenv("NO_COLOR", "1")
        report = EnvReport(
            passed=True,
            required_results=[],
            optional_results=[],
            warnings=["watch out"],
        )
        output = format_report(report)
        assert "WARNING: watch out" in output

    def test_format_report_optional_section(self, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.setenv("NO_COLOR", "1")
        report = EnvReport(
            passed=True,
            required_results=[],
            optional_results=[self._defaulted_var()],
            warnings=[],
        )
        output = format_report(report)
        assert "Optional:" in output


class TestFormatReportJson:
    def test_format_report_json_valid(self) -> None:
        report = EnvReport(
            passed=True,
            required_results=[VarResult(key="A", status=VarStatus.PASS)],
            optional_results=[VarResult(key="B", status=VarStatus.DEFAULTED)],
            warnings=["w1"],
        )
        data = json.loads(format_report_json(report))
        assert "passed" in data
        assert "required" in data
        assert "optional" in data
        assert "warnings" in data

    def test_format_report_json_fields(self) -> None:
        report = EnvReport(
            passed=True,
            required_results=[
                VarResult(
                    key="K",
                    status=VarStatus.PASS,
                    description="desc",
                    hint="h",
                    source="env",
                    message="msg",
                )
            ],
            optional_results=[],
            warnings=[],
        )
        data = json.loads(format_report_json(report))
        entry = data["required"][0]
        for field in ("key", "status", "description", "hint", "source", "message"):
            assert field in entry, f"Missing field {field}"
