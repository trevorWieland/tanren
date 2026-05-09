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
    AccountId, Email, IdempotencyKey, Identifier, InvitationToken, MembershipId, OrgId,
    OrganizationName, OrganizationPermission, SessionToken,
};

use crate::{
    AccountRecord, EventEnvelope, InvitationRecord, NewAccount, OrganizationRecord, SessionRecord,
    StoreError,
};

/// Context the store passes back to the caller's event-builder so
/// the caller can stamp the inviting org id (only known after the
/// in-transaction `consume_invitation` step) into the success-path
/// event payloads it owns.
#[derive(Debug, Clone)]
pub struct AcceptInvitationEventContext {
    /// The id of the freshly inserted account row.
    pub account_id: AccountId,
    /// The user-facing identifier on the new account.
    pub identifier: Identifier,
    /// The invitation token that was consumed.
    pub token: InvitationToken,
    /// The organization the new account joined (read from the
    /// consumed invitation row inside the transaction).
    pub joined_org: OrgId,
    /// `now` from the request, threaded through so event payloads
    /// can carry the same instant as the row writes.
    pub now: DateTime<Utc>,
}

/// Closure the store invokes inside the transaction to build the
/// success-path event envelopes. The store crate does not know the
/// concrete event payload shape — that lives in `tanren-app-services`
/// — so the caller hands in an event-builder closure and the store
/// invokes it once it has computed the inviting-org id.
pub type AcceptInvitationEventsBuilder =
    Box<dyn FnOnce(&AcceptInvitationEventContext) -> Vec<serde_json::Value> + Send>;

/// Input shape for [`AccountStore::accept_invitation_atomic`]. Bundles
/// every input the atomic flow needs so the trait method runs as a
/// single unit. The caller pre-derives the password PHC and the
/// session token because the verifier and the CSPRNG live in the
/// app-service layer, not in the store.
pub struct AcceptInvitationAtomicRequest {
    /// The invitation token the caller is trying to consume.
    pub token: InvitationToken,
    /// Wall-clock instant the flow runs at. Used for the consume
    /// predicate, the membership row, the session row, and every
    /// emitted event envelope.
    pub now: DateTime<Utc>,
    /// New account row to insert. The caller has already derived the
    /// password PHC and validated the identifier. The `org_id` field
    /// is ignored by the atomic call — the inviting org id from the
    /// consumed invitation row is the source of truth.
    pub account: NewAccount,
    /// Stable membership id the caller pre-allocates so the success
    /// path of the atomic call does not need to thread an id back
    /// out for any subsequent step.
    pub membership_id: MembershipId,
    /// Session token the caller pre-generated.
    pub session_token: SessionToken,
    /// Wall-clock time the session expires (`now + lifetime`).
    pub session_expires_at: DateTime<Utc>,
    /// Closure the store invokes inside the transaction to build the
    /// success-path event envelopes. See
    /// [`AcceptInvitationEventsBuilder`].
    pub events_builder: AcceptInvitationEventsBuilder,
}

impl std::fmt::Debug for AcceptInvitationAtomicRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcceptInvitationAtomicRequest")
            .field("token", &self.token)
            .field("now", &self.now)
            .field("account", &self.account)
            .field("membership_id", &self.membership_id)
            .field("session_expires_at", &self.session_expires_at)
            .finish_non_exhaustive()
    }
}

/// Successful return from [`AccountStore::accept_invitation_atomic`].
/// The store layer builds and appends the success-path events itself
/// (it is the only layer that knows the inviting org id at envelope-
/// build time inside the transaction); the caller only needs the
/// row-shaped output to render the response.
#[derive(Debug, Clone)]
pub struct AcceptInvitationAtomicOutput {
    /// The freshly inserted account row.
    pub account: AccountRecord,
    /// The freshly inserted session row.
    pub session: SessionRecord,
    /// Organization the new account joined.
    pub joined_org: OrgId,
}

/// Failure taxonomy for [`AccountStore::accept_invitation_atomic`]. The
/// app-service layer maps each variant to the matching
/// `AccountFailureReason`; the `Store` variant carries non-taxonomy DB
/// errors through unchanged. Mirrors [`ConsumeInvitationError`] but
/// adds [`AcceptInvitationError::DuplicateIdentifier`] for the
/// race-safe duplicate-account check that runs inside the
/// transaction.
#[derive(Debug, thiserror::Error)]
pub enum AcceptInvitationError {
    /// No invitation matches the supplied token.
    #[error("invitation not found")]
    InvitationNotFound,
    /// The invitation exists but `consumed_at` was already set.
    #[error("invitation already consumed")]
    InvitationAlreadyConsumed,
    /// The invitation exists but `expires_at <= now`.
    #[error("invitation expired")]
    InvitationExpired,
    /// The supplied identifier collides with an existing account
    /// (caught either by a pre-flight read or by the unique-index
    /// constraint inside the transaction).
    #[error("duplicate identifier")]
    DuplicateIdentifier,
    /// Unexpected database failure.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Context passed to create-organization success event builders.
#[derive(Debug, Clone)]
pub struct CreateOrganizationEventContext {
    /// Organization that was created.
    pub organization: OrganizationRecord,
    /// Account that created the organization.
    pub creator_account_id: AccountId,
    /// Membership row allocated to the creator in the organization.
    pub creator_membership_id: MembershipId,
    /// All creator permissions granted during bootstrap.
    pub granted_permissions: Vec<OrganizationPermission>,
    /// Request timestamp threaded through all writes.
    pub now: DateTime<Utc>,
}

/// Closure invoked inside the organization-create transaction to build
/// success-path event envelopes.
pub type CreateOrganizationEventsBuilder =
    Box<dyn FnOnce(&CreateOrganizationEventContext) -> Vec<serde_json::Value> + Send>;

/// Input shape for [`AccountStore::create_organization_atomic`].
pub struct CreateOrganizationAtomicRequest {
    /// Stable id allocated for the new organization.
    pub organization_id: OrgId,
    /// Normalized organization name uniqueness key.
    pub name: OrganizationName,
    /// Signed-in account creating the organization.
    pub creator_account_id: AccountId,
    /// Membership id to allocate for the creator.
    pub creator_membership_id: MembershipId,
    /// Wall-clock time for all row writes and success events.
    pub now: DateTime<Utc>,
    /// Stable client idempotency key for replay-safe create semantics.
    pub idempotency_key: Option<IdempotencyKey>,
    /// Event payload builder invoked inside the transaction.
    pub events_builder: CreateOrganizationEventsBuilder,
}

impl std::fmt::Debug for CreateOrganizationAtomicRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateOrganizationAtomicRequest")
            .field("organization_id", &self.organization_id)
            .field("name", &self.name)
            .field("creator_account_id", &self.creator_account_id)
            .field("creator_membership_id", &self.creator_membership_id)
            .field("now", &self.now)
            .field(
                "idempotency_key",
                &self
                    .idempotency_key
                    .as_ref()
                    .map_or("<none>", IdempotencyKey::as_str),
            )
            .finish_non_exhaustive()
    }
}

/// Successful return from [`AccountStore::create_organization_atomic`].
#[derive(Debug, Clone)]
pub struct CreateOrganizationAtomicOutput {
    /// Newly created organization row.
    pub organization: OrganizationRecord,
    /// Organization-level permissions granted to the creator.
    pub granted_permissions: Vec<OrganizationPermission>,
    /// New organizations own zero projects at creation time.
    pub initial_project_count: u64,
}

/// Failure taxonomy for [`AccountStore::create_organization_atomic`].
#[derive(Debug, thiserror::Error)]
pub enum CreateOrganizationError {
    /// Organization name already exists (normalized uniqueness key).
    #[error("duplicate organization name")]
    DuplicateName,
    /// The supplied idempotency key was reused with a conflicting
    /// request fingerprint.
    #[error("idempotency conflict")]
    IdempotencyConflict,
    /// Unexpected database failure.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Enforceable guard used by leave/remove-member flows so they cannot
/// orphan administrative organization permissions.
#[derive(Debug, thiserror::Error)]
pub enum LastOrganizationAdminGuardError {
    /// The account is the final holder of one or more organization
    /// administrative permissions and cannot be removed yet.
    #[error("account is last holder of administrative permissions")]
    LastAdminHolder {
        /// Permissions that would become orphaned.
        permissions: Vec<OrganizationPermission>,
    },
    /// Unexpected database failure.
    #[error(transparent)]
    Store(#[from] StoreError),
}

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
    /// **For the typical invitation-acceptance flow prefer
    /// [`AccountStore::accept_invitation_atomic`]** — it consumes the
    /// invitation, creates the account, links the membership, mints the
    /// session, and appends the success events in one transaction so a
    /// failure mid-flow leaves the invitation pending. This bare
    /// `consume_invitation` is retained for any future flow that wants
    /// to consume-without-creating.
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

    /// Run the full invitation-acceptance flow as a single transaction:
    /// consume the invitation, insert the account, link the membership,
    /// insert the session, and append the success-path
    /// `account_created` and `invitation_accepted` events. If any step
    /// fails the transaction rolls back — the invitation row stays
    /// pending so the user can retry.
    ///
    /// The caller pre-derives the password PHC, the session token, the
    /// membership id and `now`. The implementation owns the inviting-
    /// org id (it reads it from the consumed row) and stamps it onto
    /// the inserted account, the membership, and the emitted events.
    async fn accept_invitation_atomic(
        &self,
        request: AcceptInvitationAtomicRequest,
    ) -> Result<AcceptInvitationAtomicOutput, AcceptInvitationError>;

    /// Run organization creation as one transaction: insert the
    /// organization row, creator membership, all creator
    /// organization-admin grants, and success-path events.
    async fn create_organization_atomic(
        &self,
        request: CreateOrganizationAtomicRequest,
    ) -> Result<CreateOrganizationAtomicOutput, CreateOrganizationError>;

    /// Check whether an account currently holds the supplied
    /// organization-level permission.
    async fn has_organization_permission(
        &self,
        account_id: AccountId,
        org_id: OrgId,
        permission: OrganizationPermission,
    ) -> Result<bool, StoreError>;

    /// Guard future leave/remove-member flows by rejecting removal of
    /// the final administrative permission holder for the
    /// organization.
    async fn enforce_not_last_organization_admin_holder(
        &self,
        account_id: AccountId,
        org_id: OrgId,
    ) -> Result<(), LastOrganizationAdminGuardError>;

    /// Issue a session for the supplied account.
    async fn insert_session(
        &self,
        token: SessionToken,
        account_id: AccountId,
        now: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Result<SessionRecord, StoreError>;

    /// Look up a session by opaque token.
    async fn find_session_by_token(
        &self,
        token: &SessionToken,
    ) -> Result<Option<SessionRecord>, StoreError>;

    /// Look up the most recent active session for an account matching
    /// a projected expiry timestamp.
    async fn find_latest_active_session_for_account(
        &self,
        account_id: AccountId,
        expires_at: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<Option<SessionRecord>, StoreError>;

    /// List organizations currently visible to an account through
    /// membership rows.
    async fn list_organizations_for_account(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<OrganizationRecord>, StoreError>;

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
