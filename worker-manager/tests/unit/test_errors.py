"""Tests for error classification module."""

from worker_manager.errors import ErrorClass, classify_error


class TestTransientPatterns:
    def test_rate_limit(self):
        assert (
            classify_error(1, "rate limit exceeded", "", None)
            == ErrorClass.TRANSIENT
        )

    def test_rate_limit_no_space(self):
        assert (
            classify_error(1, "ratelimit hit", "", None) == ErrorClass.TRANSIENT
        )

    def test_429(self):
        assert (
            classify_error(1, "HTTP 429 Too Many Requests", "", None)
            == ErrorClass.TRANSIENT
        )

    def test_connection_refused(self):
        assert (
            classify_error(1, "", "connection refused", None)
            == ErrorClass.TRANSIENT
        )

    def test_econnreset(self):
        assert (
            classify_error(1, "Error: ECONNRESET", "", None)
            == ErrorClass.TRANSIENT
        )

    def test_etimedout(self):
        assert (
            classify_error(1, "ETIMEDOUT", "", None) == ErrorClass.TRANSIENT
        )

    def test_timeout(self):
        assert (
            classify_error(1, "request timeout after 30s", "", None)
            == ErrorClass.TRANSIENT
        )

    def test_503(self):
        assert (
            classify_error(1, "", "503 Service Unavailable", None)
            == ErrorClass.TRANSIENT
        )

    def test_service_unavailable(self):
        assert (
            classify_error(1, "service unavailable", "", None)
            == ErrorClass.TRANSIENT
        )

    def test_500(self):
        assert (
            classify_error(1, "HTTP 500 Internal Server Error", "", None)
            == ErrorClass.TRANSIENT
        )

    def test_server_error(self):
        assert (
            classify_error(1, "server error occurred", "", None)
            == ErrorClass.TRANSIENT
        )

    def test_exit_137_oom(self):
        assert classify_error(137, "", "", None) == ErrorClass.TRANSIENT


class TestFatalPatterns:
    def test_authentication_error(self):
        assert (
            classify_error(1, "authentication_error: invalid key", "", None)
            == ErrorClass.FATAL
        )

    def test_401(self):
        assert (
            classify_error(1, "HTTP 401 Unauthorized", "", None)
            == ErrorClass.FATAL
        )

    def test_permission_denied(self):
        assert (
            classify_error(1, "", "permission denied", None)
            == ErrorClass.FATAL
        )

    def test_403(self):
        assert (
            classify_error(1, "HTTP 403 Forbidden", "", None)
            == ErrorClass.FATAL
        )

    def test_command_not_found(self):
        assert (
            classify_error(127, "", "command not found", None)
            == ErrorClass.FATAL
        )

    def test_no_such_file(self):
        assert (
            classify_error(1, "", "No such file or directory", None)
            == ErrorClass.FATAL
        )

    def test_agent_error_signal(self):
        assert classify_error(1, "", "", "error") == ErrorClass.FATAL


class TestAmbiguous:
    def test_unrecognized_exit_code(self):
        assert classify_error(1, "", "", None) == ErrorClass.AMBIGUOUS

    def test_no_output(self):
        assert classify_error(42, "", "", None) == ErrorClass.AMBIGUOUS

    def test_unknown_signal(self):
        assert classify_error(1, "", "", "unknown") == ErrorClass.AMBIGUOUS


class TestPrecedence:
    def test_transient_pattern_wins_over_fatal_exit(self):
        """Transient pattern in output takes priority since pattern matching runs first."""
        assert (
            classify_error(1, "rate limit", "permission denied", None)
            == ErrorClass.TRANSIENT
        )

    def test_error_signal_overrides_transient_output(self):
        """Agent signal=error is checked first."""
        assert (
            classify_error(1, "rate limit", "", "error") == ErrorClass.FATAL
        )

    def test_exit_137_checked_before_patterns(self):
        """Exit 137 is checked before pattern matching."""
        assert (
            classify_error(137, "permission denied", "", None)
            == ErrorClass.TRANSIENT
        )
