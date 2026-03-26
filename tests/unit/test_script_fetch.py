"""Tests for the bootstrap script URL fetcher."""

from __future__ import annotations

from unittest.mock import MagicMock, patch
from urllib.parse import urlparse

import pytest

from tanren_core.adapters.script_fetch import fetch_script


@pytest.fixture(autouse=True)
def _clear_fetch_cache() -> None:
    """Clear the LRU cache between tests so each test gets a fresh fetch."""
    fetch_script.cache_clear()


def _mock_url(url: str) -> MagicMock:
    """Create a mock httpx.URL with a working .scheme attribute."""
    parsed = urlparse(url)
    mock = MagicMock()
    mock.scheme = parsed.scheme
    return mock


class TestFetchHttps:
    def test_success(self) -> None:
        mock_response = MagicMock()
        mock_response.status_code = 200
        mock_response.text = "#!/bin/bash\necho hello"
        mock_response.url = _mock_url("https://example.com/setup.sh")

        mock_httpx = MagicMock()
        mock_httpx.get.return_value = mock_response

        with patch("tanren_core.adapters.script_fetch._import_httpx", return_value=mock_httpx):
            result = fetch_script("https://example.com/setup.sh")

        assert result == "#!/bin/bash\necho hello"
        mock_httpx.get.assert_called_once_with(
            "https://example.com/setup.sh", timeout=60, follow_redirects=True
        )

    def test_http_error_raises(self) -> None:
        mock_response = MagicMock()
        mock_response.status_code = 404
        mock_response.url = _mock_url("https://example.com/missing.sh")

        mock_httpx = MagicMock()
        mock_httpx.get.return_value = mock_response

        with (
            patch("tanren_core.adapters.script_fetch._import_httpx", return_value=mock_httpx),
            pytest.raises(RuntimeError, match="HTTP 404"),
        ):
            fetch_script("https://example.com/missing.sh")

    def test_redirect_to_http_rejected(self) -> None:
        mock_response = MagicMock()
        mock_response.status_code = 200
        mock_response.url = _mock_url("http://evil.com/setup.sh")

        mock_httpx = MagicMock()
        mock_httpx.get.return_value = mock_response

        with (
            patch("tanren_core.adapters.script_fetch._import_httpx", return_value=mock_httpx),
            pytest.raises(RuntimeError, match="non-HTTPS"),
        ):
            fetch_script("https://example.com/redirected.sh")


class TestFetchGcs:
    def test_success(self) -> None:
        mock_blob = MagicMock()
        mock_blob.download_as_text.return_value = "#!/bin/bash\napt install nginx"

        mock_bucket = MagicMock()
        mock_bucket.blob.return_value = mock_blob

        mock_client = MagicMock()
        mock_client.bucket.return_value = mock_bucket

        mock_storage = MagicMock()
        mock_storage.Client.return_value = mock_client

        with patch("tanren_core.adapters.script_fetch._import_storage", return_value=mock_storage):
            result = fetch_script("gs://my-bucket/scripts/setup.sh")

        assert result == "#!/bin/bash\napt install nginx"
        mock_client.bucket.assert_called_once_with("my-bucket")
        mock_bucket.blob.assert_called_once_with("scripts/setup.sh")

    def test_nested_path(self) -> None:
        mock_blob = MagicMock()
        mock_blob.download_as_text.return_value = "script"

        mock_bucket = MagicMock()
        mock_bucket.blob.return_value = mock_blob

        mock_client = MagicMock()
        mock_client.bucket.return_value = mock_bucket

        mock_storage = MagicMock()
        mock_storage.Client.return_value = mock_client

        with patch("tanren_core.adapters.script_fetch._import_storage", return_value=mock_storage):
            fetch_script("gs://bucket/path/to/deep/script.sh")

        mock_bucket.blob.assert_called_once_with("path/to/deep/script.sh")


class TestUnsupportedScheme:
    def test_ftp_raises(self) -> None:
        with pytest.raises(ValueError, match="Unsupported URL scheme 'ftp'"):
            fetch_script("ftp://example.com/setup.sh")

    def test_http_raises(self) -> None:
        with pytest.raises(ValueError, match="Unsupported URL scheme 'http'"):
            fetch_script("http://example.com/setup.sh")

    def test_empty_scheme_raises(self) -> None:
        with pytest.raises(ValueError, match="Unsupported URL scheme"):
            fetch_script("no-scheme-at-all")
