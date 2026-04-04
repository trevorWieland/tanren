"""Postgres URL detection utility."""

from __future__ import annotations


def is_postgres_url(url: str | None) -> bool:
    """Return True if *url* looks like a Postgres DSN.

    Args:
        url: Database URL to check.

    Returns:
        True if the URL starts with ``postgresql://`` or ``postgres://``.
    """
    if not url:
        return False
    return url.lower().startswith(("postgresql://", "postgres://"))
