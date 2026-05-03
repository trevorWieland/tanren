//! Port for Tanren's account-flow persistence.
//!
//! `AccountStore` is the **port** that `tanren-app-services` consumes;
//! [`crate::Store`] is the SeaORM-backed adapter implementation. The trait
//! lives here so handlers can take `&dyn AccountStore` without knowing
//! about `SeaORM`, and so test/wire harnesses can substitute alternative
//! implementations without touching handler code.
//!
//! Design notes
//!
//! - **Sign-up and accept-invitation each touch 4-5 store operations as
//!   one unit.** Splitting into per-aggregate traits would force every
//!   handler to take a fistful of trait objects. R-0001 ships exactly one
//!   port; the worked example in
//!   `profiles/rust-cargo/architecture/trait-based-abstraction.md`
//!   reflects that decision.
//! - **No clock methods.** Every write that needs the current time takes
//!   `now: DateTime<Utc>` as a parameter; callers thread it from an
//!   injected [`crate::Clock`] equivalent. The store does not read time
//!   directly. Enforced workspace-wide for `tanren-store` by the
//!   `chrono::Utc::now` clippy denial in `clippy.toml`.
//! - **Atomic invitation consume.** The trait's `consume_invitation`
//!   contract is single-call; implementations are expected to use a
//!   single conditional UPDATE (filtered on `consumed_at IS NULL` and
//!   `expires_at > now`) so concurrent acceptances of the same token
//!   serialize to exactly one success.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tanren_identity_policy::{
    AccountId, Email, Identifier, InvitationToken, MembershipId, OrgId, SessionToken,
};

use crate::{
    AccountRecord, EventEnvelope, InvitationRecord, NewAccount, SessionRecord, StoreError,
};

/// Port the account-flow handlers consume. The SeaORM-backed adapter is
/// `impl AccountStore for Store` (see `lib.rs`).
#[async_trait]
pub trait AccountStore: Send + Sync + std::fmt::Debug {
    /// Look up an account by its case-sensitive identifier.
    async fn find_account_by_identifier(
        &self,
        identifier: &Identifier,
    ) -> Result<Option<AccountRecord>, StoreError>;

    /// Look up an account by an [`Email`]. Equivalent to
    /// `find_account_by_identifier(&Identifier::from_email(email))`;
    /// kept on the trait so the email-driven sign-in path reads
    /// naturally and so adapters that prefer to query by a different
    /// derived column can override the implementation.
    async fn find_account_by_email(
        &self,
        email: &Email,
    ) -> Result<Option<AccountRecord>, StoreError>;

    /// Insert a new account row.
    async fn insert_account(&self, new: NewAccount) -> Result<AccountRecord, StoreError>;

    /// Insert a membership linking an account to an organization at the
    /// supplied instant.
    async fn insert_membership(
        &self,
        account_id: AccountId,
        org_id: OrgId,
        now: DateTime<Utc>,
    ) -> Result<MembershipId, StoreError>;

    /// Look up an invitation by token.
    async fn find_invitation_by_token(
        &self,
        token: &InvitationToken,
    ) -> Result<Option<InvitationRecord>, StoreError>;

    /// Atomically consume an invitation. Implementations issue a single
    /// conditional UPDATE filtered on `consumed_at IS NULL AND expires_at
    /// > now` and use a follow-up read to populate the return shape and
    /// disambiguate the failure taxonomy.
    ///
    /// Returns:
    /// - `Ok(ConsumedInvitation { .. })` when exactly one row was
    ///   transitioned.
    /// - `Err(ConsumeInvitationError::NotFound)` when no row matches the
    ///   token at all.
    /// - `Err(ConsumeInvitationError::AlreadyConsumed)` when the row
    ///   exists but `consumed_at` was already set by another caller.
    /// - `Err(ConsumeInvitationError::Expired)` when the row exists but
    ///   `expires_at <= now`.
    /// - `Err(ConsumeInvitationError::Store(_))` for unexpected DB
    ///   failures.
    async fn consume_invitation(
        &self,
        token: &InvitationToken,
        now: DateTime<Utc>,
    ) -> Result<ConsumedInvitation, ConsumeInvitationError>;

    /// Issue a session for the supplied account.
    async fn insert_session(
        &self,
        token: SessionToken,
        account_id: AccountId,
        now: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Result<SessionRecord, StoreError>;

    /// Append a payload to the canonical event log at the supplied
    /// instant.
    async fn append_event(
        &self,
        payload: serde_json::Value,
        now: DateTime<Utc>,
    ) -> Result<EventEnvelope, StoreError>;

    /// Read the most recent `limit` events, newest first.
    async fn recent_events(&self, limit: u64) -> Result<Vec<EventEnvelope>, StoreError>;
}

/// Successful return from [`AccountStore::consume_invitation`].
#[derive(Debug, Clone)]
pub struct ConsumedInvitation {
    /// Organization the new account joins on acceptance.
    pub inviting_org_id: OrgId,
    /// Wall-clock time the invitation was set to expire.
    pub expires_at: DateTime<Utc>,
    /// Wall-clock time the invitation was consumed (the `now` passed in).
    pub consumed_at: DateTime<Utc>,
}

/// Failure taxonomy for [`AccountStore::consume_invitation`]. The
/// app-service layer maps each variant to the matching
/// `AccountFailureReason`; the `Store` variant carries non-taxonomy DB
/// errors through unchanged.
#[derive(Debug, thiserror::Error)]
pub enum ConsumeInvitationError {
    /// No invitation matches the supplied token.
    #[error("invitation not found")]
    NotFound,
    /// The invitation exists but `consumed_at` was already set.
    #[error("invitation already consumed")]
    AlreadyConsumed,
    /// The invitation exists but `expires_at <= now`.
    #[error("invitation expired")]
    Expired,
    /// Unexpected database failure.
    #[error(transparent)]
    Store(#[from] StoreError),
}
