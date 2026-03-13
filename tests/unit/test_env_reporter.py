"""Tests for env reporter module."""

import json

from tanren_core.env.reporter import format_report, format_report_json
from tanren_core.env.validator import EnvReport, VarResult, VarStatus


def _make_report(passed=True, required=None, optional=None, warnings=None):
    return EnvReport(
        passed=passed,
        required_results=required or [],
        optional_results=optional or [],
        warnings=warnings or [],
    )


class TestFormatReport:
    def test_passed_report(self):
        report = _make_report(
            passed=True,
            required=[VarResult(key="API_KEY", status=VarStatus.PASS)],
        )
        text = format_report(report, "myproject", "tanren.yml")
        assert "myproject" in text
        assert "PASSED" in text

    def test_failed_report(self):
        report = _make_report(
            passed=False,
            required=[
                VarResult(
                    key="API_KEY",
                    status=VarStatus.MISSING,
                    hint="Get one at example.com",
                )
            ],
        )
        text = format_report(report, "myproject")
        assert "FAILED" in text
        assert "API_KEY" in text
        assert "hint:" in text

    def test_verbose_shows_passing(self):
        report = _make_report(
            passed=True,
            required=[VarResult(key="API_KEY", status=VarStatus.PASS, source=".env")],
        )
        text = format_report(report, verbose=True)
        assert "API_KEY" in text

    def test_non_verbose_hides_passing(self):
        report = _make_report(
            passed=True,
            required=[VarResult(key="API_KEY", status=VarStatus.PASS)],
        )
        text = format_report(report, verbose=False)
        assert "1 passed" in text

    def test_defaulted_shown(self):
        report = _make_report(
            optional=[
                VarResult(
                    key="LOG_LEVEL",
                    status=VarStatus.DEFAULTED,
                    source="default",
                    message="Using default: INFO",
                )
            ],
        )
        text = format_report(report, verbose=False)
        assert "LOG_LEVEL" in text
        assert "Using default" in text


class TestFormatReportJson:
    def test_json_structure(self):
        report = _make_report(
            passed=False,
            required=[
                VarResult(key="K", status=VarStatus.MISSING, hint="h"),
            ],
            warnings=["a warning"],
        )
        raw = format_report_json(report)
        data = json.loads(raw)
        assert data["passed"] is False
        assert len(data["required"]) == 1
        assert data["required"][0]["key"] == "K"
        assert data["required"][0]["status"] == "missing"
        assert data["warnings"] == ["a warning"]
