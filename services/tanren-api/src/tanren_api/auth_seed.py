"""Idempotent seeding of the legacy admin user and API key.

On startup, if ``TANREN_API_API_KEY`` is set, creates an admin user and
API key with ``*`` scope.  If the key hash changes (operator rotated
the env var), the old key is revoked and a new one is seeded.

Safe for concurrent startup: IntegrityError on duplicate user/key is
treated as a successful no-op (another worker won the race).  Revoked
key hashes are treated as not-seeded so re-setting a previously rotated
key value works correctly.
"""

from __future__ import annotations

import json
import logging
import uuid

from tanren_api.key_utils import hash_api_key
from tanren_core.store.auth_events import KeyCreated, UserCreated
from tanren_core.store.auth_protocols import AuthStore
from tanren_core.store.protocols import EventStore
from tanren_core.timestamps import utc_now_iso

logger = logging.getLogger(__name__)

LEGACY_ADMIN_USER_ID = "admin-00000000"


async def seed_legacy_admin_key(
    auth_store: AuthStore,
    event_store: EventStore,
    raw_key: str,
) -> None:
    """Create admin user + API key from the legacy ``TANREN_API_API_KEY`` env var.

    Idempotent: skips if a non-revoked key with the same hash already
    exists.  If a different legacy key was previously seeded, the old key
    is revoked before the new one is created, ensuring rotated env vars
    invalidate prior credentials.
    """
    key_hash = hash_api_key(raw_key)

    # Skip only if the key exists AND is not revoked
    existing = await auth_store.get_api_key_by_hash(key_hash)
    if existing is not None and existing.revoked_at is None:
        logger.debug("Legacy admin key already seeded — skipping")
        return

    now = utc_now_iso()

    # Ensure admin user exists (race-safe: catch duplicate insert)
    user = await auth_store.get_user(LEGACY_ADMIN_USER_ID)
    if user is None:
        try:
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
        except Exception:
            # Concurrent startup: another worker may have created the user.
            if await auth_store.get_user(LEGACY_ADMIN_USER_ID) is not None:
                logger.debug("Legacy admin user created by concurrent worker")
            else:
                raise

    # Revoke any previously seeded legacy keys for this admin user.
    # This handles env var rotation: old key becomes invalid immediately.
    existing_keys = await auth_store.list_api_keys(
        user_id=LEGACY_ADMIN_USER_ID, include_revoked=False
    )
    for old_key in existing_keys:
        if old_key.name == "Legacy admin key":
            await auth_store.revoke_api_key(old_key.key_id)
            logger.info("Revoked old legacy admin key %s (env var rotated)", old_key.key_prefix)

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

    try:
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
    except Exception:
        # Concurrent startup: another worker may have inserted first.
        if await auth_store.get_api_key_by_hash(key_hash) is not None:
            logger.debug("Legacy admin key seeded by concurrent worker — skipping")
            return
        raise

    logger.info("Seeded legacy admin user and API key")
