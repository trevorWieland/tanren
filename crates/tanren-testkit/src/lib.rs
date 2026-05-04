//! Shared test utilities for the Tanren BDD harness.
//!
//! This crate is intentionally not pulled into product code. Only the BDD
//! step-definition crate depends on it. Per architecture, test-only support
//! must not leak into runtime crates.
//!
//! Every public item is gated behind the `test-hooks` Cargo feature so
//! the `xtask check-test-hooks` guard treats the whole crate as
//! correctly-scoped fixture surface. The feature is on by default;
//! consumers that disable default features get an empty crate.

#![cfg(feature = "test-hooks")]

pub mod harness;

pub use harness::{
    AccountHarness, ActorState, ApiHarness, CliHarness, ConcurrentAcceptanceTally,
    HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind, HarnessOrgCreation,
    HarnessOutcome, HarnessResult, HarnessSession, InProcessHarness, McpHarness, TuiHarness,
    WebHarness, event_kinds, record_failure,
};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use tanren_app_services::Store;
use tanren_identity_policy::{InvitationToken, OrgId};
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

/// Generate a fresh fixture invitation token. The output is a 36-byte
/// `inv-<uuid>` string — well above the 16-byte `InvitationToken` floor.
///
/// # Panics
///
/// Panics only if the resulting string fails [`InvitationToken::parse`],
/// which would indicate a programmer error in this helper rather than
/// runtime input — the panic is the right signal.
#[must_use]
pub fn fresh_invitation_token() -> InvitationToken {
    let raw = format!("inv-{}", Uuid::new_v4().simple());
    InvitationToken::parse(&raw).expect("synthesised invitation token must parse")
}

/// Spec for a test invitation. R-0005 will land the user-facing
/// invitation-send flow; R-0001's BDD scenarios seed pending invitations
/// directly via this helper.
#[derive(Debug, Clone)]
pub struct InvitationFixture {
    /// The opaque token callers will accept against.
    pub token: InvitationToken,
    /// Inviting organization id.
    pub inviting_org: OrgId,
    /// Expiry instant.
    pub expires_at: DateTime<Utc>,
}

impl InvitationFixture {
    /// A fresh, valid invitation expiring one day after `now`.
    #[must_use]
    pub fn valid(now: DateTime<Utc>) -> Self {
        Self {
            token: fresh_invitation_token(),
            inviting_org: OrgId::fresh(),
            expires_at: now + Duration::days(1),
        }
    }

    /// An already-expired invitation (expired one second before `now`).
    #[must_use]
    pub fn expired(now: DateTime<Utc>) -> Self {
        Self {
            token: fresh_invitation_token(),
            inviting_org: OrgId::fresh(),
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
