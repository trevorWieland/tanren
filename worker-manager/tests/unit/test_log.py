"""Tests for structured logging module."""

import logging

from worker_manager.log import (
    PREFIX,
    TanrenFormatter,
    configure_logging,
    phase_banner,
    phase_complete,
    supports_color,
)


class TestSupportsColor:
    def test_no_color_env(self, monkeypatch):
        monkeypatch.setenv("NO_COLOR", "1")
        assert supports_color() is False

    def test_without_no_color(self, monkeypatch):
        monkeypatch.delenv("NO_COLOR", raising=False)
        # In test environment, stderr might not be a TTY
        result = supports_color()
        assert isinstance(result, bool)


class TestTanrenFormatter:
    def test_format_includes_prefix(self):
        formatter = TanrenFormatter()
        record = logging.LogRecord(
            name="test", level=logging.INFO, pathname="", lineno=0,
            msg="hello world", args=(), exc_info=None,
        )
        output = formatter.format(record)
        assert output.startswith(PREFIX)
        assert "INFO" in output
        assert "hello world" in output


class TestConfigureLogging:
    def test_configures_root_logger(self):
        configure_logging("DEBUG")
        root = logging.getLogger()
        assert root.level == logging.DEBUG
        # Clean up
        configure_logging("WARNING")

    def test_env_override(self, monkeypatch):
        monkeypatch.setenv("TANREN_LOG_LEVEL", "ERROR")
        configure_logging("DEBUG")  # should be overridden by env
        root = logging.getLogger()
        assert root.level == logging.ERROR
        configure_logging("WARNING")


class TestPhaseBanner:
    def test_logs_banner(self, caplog):
        with caplog.at_level(logging.INFO, logger="tanren"):
            phase_banner("preflight", project="myproject")
        assert "PREFLIGHT" in caplog.text
        assert "myproject" in caplog.text

    def test_with_spec(self, caplog):
        with caplog.at_level(logging.INFO, logger="tanren"):
            phase_banner("do-task", spec="tanren/specs/test")
        assert "DO-TASK" in caplog.text


class TestPhaseComplete:
    def test_logs_complete(self, caplog):
        with caplog.at_level(logging.INFO, logger="tanren"):
            phase_complete("preflight", 12.3, outcome="success")
        assert "COMPLETE" in caplog.text
        assert "12.3s" in caplog.text
