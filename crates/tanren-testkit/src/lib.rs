//! Shared test utilities for the Tanren BDD harness.
//!
//! This crate is intentionally not pulled into product code. Only the BDD
//! step-definition crate depends on it. Per architecture, test-only support
//! must not leak into runtime crates.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use tanren_app_services::Store;
use tanren_store::{NewInvitation, StoreError};
use uuid::Uuid;

/// A fixture seed used to produce deterministic test data. Default seed is
/// `0` so unset fixtures still serialize stably across runs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureSeed(u64);

impl FixtureSeed {
    /// Wrap a numeric seed.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// The numeric seed value.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Connect a fresh in-memory `SQLite` store and apply all migrations. The
/// returned [`Store`] is the same shape the production binaries use; tests
/// drive it through `tanren-app-services::Handlers`.
///
/// # Errors
///
/// Returns [`StoreError::Database`] if the connection or migration fails.
pub async fn ephemeral_store() -> Result<Store, StoreError> {
    let store = Store::connect("sqlite::memory:").await?;
    store.migrate().await?;
    Ok(store)
}

/// Spec for a test invitation. R-0005 will land the user-facing
/// invitation-send flow; R-0001's BDD scenarios seed pending invitations
/// directly via this helper.
#[derive(Debug, Clone)]
pub struct InvitationFixture {
    /// The opaque token callers will accept against.
    pub token: String,
    /// Inviting organization id.
    pub inviting_org: Uuid,
    /// Expiry instant.
    pub expires_at: DateTime<Utc>,
}

impl InvitationFixture {
    /// A fresh, valid invitation expiring one day after `now`.
    #[must_use]
    pub fn valid(now: DateTime<Utc>) -> Self {
        Self {
            token: format!("inv-{}", Uuid::new_v4().simple()),
            inviting_org: Uuid::now_v7(),
            expires_at: now + Duration::days(1),
        }
    }

    /// An already-expired invitation (expired one second before `now`).
    #[must_use]
    pub fn expired(now: DateTime<Utc>) -> Self {
        Self {
            token: format!("inv-{}", Uuid::new_v4().simple()),
            inviting_org: Uuid::now_v7(),
            expires_at: now - Duration::seconds(1),
        }
    }
}

/// Persist a fixture invitation into the store.
///
/// # Errors
///
/// Returns [`StoreError::Database`] if the insert fails.
pub async fn seed_invitation(store: &Store, fixture: &InvitationFixture) -> Result<(), StoreError> {
    store
        .seed_invitation(NewInvitation {
            token: fixture.token.clone(),
            inviting_org_id: fixture.inviting_org,
            expires_at: fixture.expires_at,
        })
        .await?;
    Ok(())
}
