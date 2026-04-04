"""Shared UTC timestamp utilities.

Single source of truth for ISO 8601 timestamp generation.  All store
operations, event emission, and projection updates should use
:func:`utc_now_iso` for consistent formatting (``Z`` suffix, no
``+00:00``).
"""

from __future__ import annotations

from datetime import UTC, datetime


def utc_now_iso() -> str:
    """Return the current UTC time as an ISO 8601 string with Z suffix.

    Returns:
        Timestamp like ``2026-01-15T12:30:45.123456Z``.
    """
    return datetime.now(UTC).isoformat().replace("+00:00", "Z")
