"""Tests for Postgres URL detection."""

from __future__ import annotations

from tanren_core.adapters.postgres_pool import is_postgres_url


class TestIsPostgresUrl:
    def test_postgresql_scheme(self):
        assert is_postgres_url("postgresql://localhost/db") is True

    def test_postgres_scheme(self):
        assert is_postgres_url("postgres://localhost/db") is True

    def test_sqlite_path(self):
        assert is_postgres_url("/data/events.db") is False

    def test_none(self):
        assert is_postgres_url(None) is False

    def test_empty_string(self):
        assert is_postgres_url("") is False

    def test_relative_path(self):
        assert is_postgres_url("events.db") is False

    def test_uppercase_postgresql_scheme(self):
        assert is_postgres_url("POSTGRESQL://localhost/db") is True

    def test_mixed_case_postgres_scheme(self):
        assert is_postgres_url("Postgres://localhost/db") is True
