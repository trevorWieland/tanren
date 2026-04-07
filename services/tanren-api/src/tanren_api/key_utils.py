"""API key generation and hashing utilities."""

from __future__ import annotations

import hashlib
import secrets


def generate_api_key() -> tuple[str, str, str]:
    """Generate a new API key.

    Returns:
        Tuple of ``(full_key, prefix, key_hash)``:
        - ``full_key``: ``tnrn_{prefix}_{random}`` — returned to user ONCE
        - ``prefix``: 8-char hex prefix for identification/listing
        - ``key_hash``: SHA-256 hex digest for storage/verification
    """
    prefix = secrets.token_hex(4)  # 8 hex chars
    random_part = secrets.token_urlsafe(32)
    full_key = f"tnrn_{prefix}_{random_part}"
    key_hash = hash_api_key(full_key)
    return full_key, prefix, key_hash


def hash_api_key(key: str) -> str:
    """Hash an API key with SHA-256 for storage/lookup.

    API keys are high-entropy random strings, so SHA-256 is secure and fast
    (no need for bcrypt/scrypt which are designed for low-entropy passwords).
    """
    return hashlib.sha256(key.encode()).hexdigest()
