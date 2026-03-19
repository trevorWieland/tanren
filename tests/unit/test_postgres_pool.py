"""Tests for the Postgres pool factory and URL detection."""

from __future__ import annotations

from unittest.mock import AsyncMock, MagicMock, patch

from tanren_core.adapters.postgres_pool import create_postgres_pool, is_postgres_url


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


class TestCreatePool:
    @patch("tanren_core.adapters.postgres_pool.ensure_schema", new_callable=AsyncMock)
    @patch("tanren_core.adapters.postgres_pool.asyncpg.create_pool", new_callable=AsyncMock)
    async def test_create_pool_calls_ensure_schema(self, mock_create_pool, mock_ensure_schema):
        mock_pool = MagicMock()
        mock_create_pool.return_value = mock_pool

        result = await create_postgres_pool("postgresql://localhost/db")

        assert result is mock_pool
        mock_create_pool.assert_awaited_once_with(
            "postgresql://localhost/db", min_size=2, max_size=10
        )
        mock_ensure_schema.assert_awaited_once_with(mock_pool)

    @patch("tanren_core.adapters.postgres_pool.ensure_schema", new_callable=AsyncMock)
    @patch("tanren_core.adapters.postgres_pool.asyncpg.create_pool", new_callable=AsyncMock)
    async def test_create_pool_custom_sizes(self, mock_create_pool, mock_ensure_schema):
        mock_pool = MagicMock()
        mock_create_pool.return_value = mock_pool

        await create_postgres_pool("postgresql://localhost/db", min_size=1, max_size=5)

        mock_create_pool.assert_awaited_once_with(
            "postgresql://localhost/db", min_size=1, max_size=5
        )
