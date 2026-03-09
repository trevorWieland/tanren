"""Tests for env validator module."""

from worker_manager.env.schema import EnvBlock, OptionalEnvVar, RequiredEnvVar
from worker_manager.env.validator import VarStatus, validate_env


class TestValidateRequired:
    def test_pass(self):
        block = EnvBlock(required=[RequiredEnvVar(key="API_KEY")])
        merged = {"API_KEY": "sk-abc123"}
        source = {"API_KEY": ".env"}
        report = validate_env(block, merged, source)
        assert report.passed
        assert len(report.required_results) == 1
        assert report.required_results[0].status == VarStatus.PASS

    def test_missing(self):
        block = EnvBlock(required=[RequiredEnvVar(key="API_KEY", hint="get one")])
        report = validate_env(block, {}, {})
        assert not report.passed
        assert report.required_results[0].status == VarStatus.MISSING
        assert report.required_results[0].hint == "get one"

    def test_empty(self):
        block = EnvBlock(required=[RequiredEnvVar(key="API_KEY")])
        report = validate_env(block, {"API_KEY": ""}, {"API_KEY": ".env"})
        assert not report.passed
        assert report.required_results[0].status == VarStatus.EMPTY

    def test_pattern_match(self):
        block = EnvBlock(
            required=[RequiredEnvVar(key="K", pattern="^sk-or-v1-")]
        )
        report = validate_env(block, {"K": "sk-or-v1-abc123"}, {"K": ".env"})
        assert report.passed
        assert report.required_results[0].status == VarStatus.PASS

    def test_pattern_mismatch(self):
        block = EnvBlock(
            required=[RequiredEnvVar(key="K", pattern="^sk-or-v1-")]
        )
        report = validate_env(block, {"K": "wrong-prefix"}, {"K": ".env"})
        assert not report.passed
        assert report.required_results[0].status == VarStatus.PATTERN_MISMATCH
        # Value should be redacted in message
        assert "wron..." in report.required_results[0].message

    def test_multiple_required_partial_fail(self):
        block = EnvBlock(
            required=[
                RequiredEnvVar(key="A"),
                RequiredEnvVar(key="B"),
            ]
        )
        report = validate_env(block, {"A": "val"}, {"A": ".env"})
        assert not report.passed
        statuses = {r.key: r.status for r in report.required_results}
        assert statuses["A"] == VarStatus.PASS
        assert statuses["B"] == VarStatus.MISSING

    def test_all_pass(self):
        block = EnvBlock(
            required=[
                RequiredEnvVar(key="A"),
                RequiredEnvVar(key="B"),
            ]
        )
        merged = {"A": "1", "B": "2"}
        source = {"A": ".env", "B": ".env"}
        report = validate_env(block, merged, source)
        assert report.passed

    def test_from_os_environ(self, monkeypatch):
        monkeypatch.setenv("API_KEY", "sk-live-123")
        block = EnvBlock(required=[RequiredEnvVar(key="API_KEY")])
        report = validate_env(block, {}, {})
        assert report.passed
        assert report.required_results[0].source == "os.environ"


class TestValidateOptional:
    def test_present(self):
        block = EnvBlock(optional=[OptionalEnvVar(key="LOG_LEVEL")])
        report = validate_env(block, {"LOG_LEVEL": "DEBUG"}, {"LOG_LEVEL": ".env"})
        assert report.passed
        assert report.optional_results[0].status == VarStatus.PASS

    def test_missing_with_default(self):
        block = EnvBlock(optional=[OptionalEnvVar(key="LOG_LEVEL", default="INFO")])
        merged: dict[str, str] = {}
        source: dict[str, str] = {}
        report = validate_env(block, merged, source)
        assert report.passed
        assert report.optional_results[0].status == VarStatus.DEFAULTED
        # Default should be injected into merged env
        assert merged["LOG_LEVEL"] == "INFO"

    def test_missing_no_default(self):
        block = EnvBlock(optional=[OptionalEnvVar(key="EXTRA")])
        report = validate_env(block, {}, {})
        assert report.passed  # optional missing is not a failure
        assert report.optional_results[0].status == VarStatus.MISSING

    def test_pattern_mismatch_warning(self):
        block = EnvBlock(
            optional=[OptionalEnvVar(key="URL", pattern="^https://")]
        )
        report = validate_env(block, {"URL": "http://bad"}, {"URL": ".env"})
        assert report.passed  # optional pattern mismatch is not a hard failure
        assert report.optional_results[0].status == VarStatus.PATTERN_MISMATCH
        assert len(report.warnings) == 1

    def test_pattern_match(self):
        block = EnvBlock(
            optional=[OptionalEnvVar(key="URL", pattern="^https://")]
        )
        report = validate_env(block, {"URL": "https://ok"}, {"URL": ".env"})
        assert report.optional_results[0].status == VarStatus.PASS


class TestRedaction:
    def test_short_value_fully_redacted(self):
        block = EnvBlock(required=[RequiredEnvVar(key="K", pattern="^abc$")])
        report = validate_env(block, {"K": "xy"}, {"K": ".env"})
        assert "****" in report.required_results[0].message

    def test_long_value_partially_redacted(self):
        block = EnvBlock(required=[RequiredEnvVar(key="K", pattern="^abc$")])
        report = validate_env(block, {"K": "xyzzzzz"}, {"K": ".env"})
        assert "xyzz..." in report.required_results[0].message


class TestEmptyBlock:
    def test_no_vars(self):
        block = EnvBlock()
        report = validate_env(block, {}, {})
        assert report.passed
        assert report.required_results == []
        assert report.optional_results == []
