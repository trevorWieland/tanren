"""Integration tests for script_fetch module — real imports, mocked I/O."""

from __future__ import annotations

from unittest.mock import MagicMock, patch

import pytest

from tanren_core.adapters.script_fetch import (
    _import_httpx,
    fetch_script,
)


class TestImportHttpx:
    def test_real_import_succeeds(self) -> None:
        """httpx is installed in dev deps — import should succeed."""
        mod = _import_httpx()
        assert hasattr(mod, "get")

    def test_import_error_when_missing(self) -> None:
        with (
            patch.dict("sys.modules", {"httpx": None}),
            pytest.raises(ImportError, match="httpx is required"),
        ):
            _import_httpx()


class TestFetchHttpsIntegration:
    def test_round_trip_with_real_httpx(self) -> None:
        """Use real httpx module but mock the network call."""
        mock_response = MagicMock()
        mock_response.status_code = 200
        mock_response.text = "#!/bin/bash\necho integration"

        with patch("tanren_core.adapters.script_fetch._import_httpx") as mock_import:
            mock_httpx = MagicMock()
            mock_httpx.get.return_value = mock_response
            mock_import.return_value = mock_httpx

            result = fetch_script("https://example.com/bootstrap.sh")

        assert result == "#!/bin/bash\necho integration"


class TestFetchGcsIntegration:
    def test_round_trip_with_mocked_storage(self) -> None:
        mock_blob = MagicMock()
        mock_blob.download_as_text.return_value = "#!/bin/bash\napt install -y nginx"

        mock_bucket = MagicMock()
        mock_bucket.blob.return_value = mock_blob

        mock_client = MagicMock()
        mock_client.bucket.return_value = mock_bucket

        mock_storage = MagicMock()
        mock_storage.Client.return_value = mock_client

        with patch("tanren_core.adapters.script_fetch._import_storage", return_value=mock_storage):
            result = fetch_script("gs://my-bucket/scripts/setup.sh")

        assert result == "#!/bin/bash\napt install -y nginx"
        mock_client.bucket.assert_called_once_with("my-bucket")
        mock_bucket.blob.assert_called_once_with("scripts/setup.sh")
