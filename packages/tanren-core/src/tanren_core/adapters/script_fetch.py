"""Fetch bootstrap script content from HTTPS or GCS URLs."""

from __future__ import annotations

import logging
from typing import TYPE_CHECKING
from urllib.parse import urlparse, urlunparse

if TYPE_CHECKING:
    import types

logger = logging.getLogger(__name__)


def _import_httpx() -> types.ModuleType:
    """Import httpx at runtime.

    Returns:
        The httpx module.

    Raises:
        ImportError: If httpx is not installed.
    """
    try:
        import httpx as _httpx  # noqa: PLC0415 — deferred import for optional dependency
    except ImportError:
        raise ImportError(
            "httpx is required for HTTPS script fetching. Install it with: uv sync --extra all"
        ) from None
    else:
        return _httpx


def _import_storage() -> types.ModuleType:
    """Import google.cloud.storage at runtime.

    Returns:
        The google.cloud.storage module.

    Raises:
        ImportError: If google-cloud-storage is not installed.
    """
    try:
        import google.cloud.storage as _storage  # noqa: PLC0415 — deferred import for optional dependency
    except ImportError:
        raise ImportError(
            "google-cloud-storage is required for gs:// script fetching. "
            "Install it with: uv sync --extra gcp"
        ) from None
    else:
        return _storage


def fetch_script(url: str) -> str:
    """Fetch bootstrap script content from an HTTPS or GCS URL.

    Args:
        url: The URL to fetch from. Supported schemes: ``https://``, ``gs://``.

    Returns:
        The script content as a string.

    Raises:
        ValueError: If the URL scheme is unsupported.
    """
    parsed = urlparse(url)

    if parsed.scheme == "https":
        return _fetch_https(url)
    if parsed.scheme == "gs":
        return _fetch_gcs(parsed.netloc, parsed.path.lstrip("/"))

    raise ValueError(
        f"Unsupported URL scheme {parsed.scheme!r} for bootstrap script. Use https:// or gs://"
    )


def _redact_url(url: str) -> str:
    """Strip query parameters and fragment from a URL for safe logging.

    Returns:
        URL with only scheme, host, and path.
    """
    parsed = urlparse(url)
    return urlunparse((parsed.scheme, parsed.netloc, parsed.path, "", "", ""))


def _fetch_https(url: str) -> str:
    """Fetch script content via HTTPS.

    Returns:
        The script content.

    Raises:
        RuntimeError: If the HTTP request fails.
    """
    httpx = _import_httpx()
    logger.info("Fetching bootstrap script from %s", _redact_url(url))
    response = httpx.get(url, timeout=60, follow_redirects=True)
    if response.status_code != 200:
        raise RuntimeError(
            f"Failed to fetch bootstrap script from {url}: HTTP {response.status_code}"
        )
    return response.text


def _fetch_gcs(bucket_name: str, blob_path: str) -> str:
    """Fetch script content from Google Cloud Storage.

    Returns:
        The script content.
    """
    storage = _import_storage()
    logger.info("Fetching bootstrap script from gs://%s/%s", bucket_name, blob_path)
    client = storage.Client()
    bucket = client.bucket(bucket_name)
    blob = bucket.blob(blob_path)
    return blob.download_as_text()
