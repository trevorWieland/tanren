"""Idempotent seeding of the legacy admin user and API key."""

from __future__ import annotations

import json
import logging
import uuid

from tanren_api.key_utils import hash_api_key
from tanren_core.store.auth_events import KeyCreated, UserCreated
from tanren_core.store.auth_protocols import AuthStore
from tanren_core.store.protocols import EventStore

logger = logging.getLogger(__name__)

LEGACY_ADMIN_USER_ID = "admin-00000000"


def _now() -> str:
    from datetime import UTC, datetime

    return datetime.now(UTC).isoformat().replace("+00:00", "Z")


async def seed_legacy_admin_key(
    auth_store: AuthStore,
    event_store: EventStore,
    raw_key: str,
) -> None:
    """Create admin user + API key from the legacy ``TANREN_API_API_KEY`` env var.

    Idempotent: skips if a key with the same hash already exists.
    """
    key_hash = hash_api_key(raw_key)

    existing = await auth_store.get_api_key_by_hash(key_hash)
    if existing is not None:
        logger.debug("Legacy admin key already seeded — skipping")
        return

    now = _now()

    # Ensure admin user exists
    user = await auth_store.get_user(LEGACY_ADMIN_USER_ID)
    if user is None:
        await event_store.append(
            UserCreated(
                timestamp=now,
                entity_id=LEGACY_ADMIN_USER_ID,
                user_id=LEGACY_ADMIN_USER_ID,
                name="Admin (legacy)",
                email=None,
                role="admin",
            )
        )
        await auth_store.create_user(
            user_id=LEGACY_ADMIN_USER_ID,
            name="Admin (legacy)",
            email=None,
            role="admin",
        )

    # Derive prefix from key format or hash
    prefix = key_hash[:8]
    if raw_key.startswith("tnrn_") and raw_key.count("_") >= 2:
        prefix = raw_key.split("_")[1][:8]

    key_id = uuid.uuid4().hex
    scopes = ["*"]

    await event_store.append(
        KeyCreated(
            timestamp=now,
            entity_id=key_id,
            key_id=key_id,
            user_id=LEGACY_ADMIN_USER_ID,
            name="Legacy admin key",
            key_prefix=prefix,
            scopes=scopes,
            resource_limits=None,
            expires_at=None,
        )
    )
    await auth_store.create_api_key(
        key_id=key_id,
        user_id=LEGACY_ADMIN_USER_ID,
        name="Legacy admin key",
        key_prefix=prefix,
        key_hash=key_hash,
        scopes_json=json.dumps(scopes),
        resource_limits_json=None,
        expires_at=None,
    )

    logger.info("Seeded legacy admin user and API key")
