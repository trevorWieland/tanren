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
    AccountId, Email, Identifier, InvitationToken, MembershipId, OrgId, ProjectId,
    ProviderConnectionId, SessionToken,
};

use crate::{
    AccountRecord, EventEnvelope, InvitationRecord, NewAccount, NewProject,
    ProjectDependencyRecord, ProjectLoopFixtureRecord, ProjectRecord, ProjectSpecRecord,
    SessionRecord, StoreError,
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
    /// > now`. For the typical invitation-acceptance flow prefer
    /// [`AccountStore::accept_invitation_atomic`].
    async fn consume_invitation(
        &self,
        token: &InvitationToken,
        now: DateTime<Utc>,
    ) -> Result<ConsumedInvitation, ConsumeInvitationError>;

    /// Full invitation-acceptance flow as a single transaction: consume
    /// the invitation, insert the account, link the membership, insert
    /// the session, and append success events. On failure the transaction
    /// rolls back. The caller pre-derives the PHC, session token,
    /// membership id, and `now`; the implementation reads the inviting
    /// org id from the consumed invitation row.
    async fn accept_invitation_atomic(
        &self,
        request: AcceptInvitationAtomicRequest,
    ) -> Result<AcceptInvitationAtomicOutput, AcceptInvitationError>;

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

/// Status of a cross-project dependency link's target. Used by
/// [`ProjectStore::read_project_dependencies`] to surface
/// unresolved-link signals without requiring M-0007's lookup module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyLinkStatus {
    /// Target project exists and is connected.
    Resolved,
    /// Target project exists but is currently disconnected.
    TargetDisconnected,
    /// Target project does not exist in the store.
    TargetUnknown,
}

/// A cross-project dependency link annotated with the resolution status
/// of its target project. Disconnected or unknown targets surface as
/// unresolved-link signals that upstream callers can render.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectDependencyLink {
    /// The dependency record itself.
    pub dependency: ProjectDependencyRecord,
    /// Whether the target project is resolved, disconnected, or unknown.
    pub status: DependencyLinkStatus,
}

/// Return shape for [`ProjectStore::reconnect_project`]. Carries the
/// restored project record alongside the retained spec history so the
/// reconnection path (B-0025) can restore access to prior specs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconnectedProject {
    /// The reconnected project record (status is now `Connected`).
    pub project: ProjectRecord,
    /// Spec records retained from before the disconnection.
    pub specs: Vec<ProjectSpecRecord>,
}

/// Failure taxonomy for [`ProjectStore::disconnect_project`].
#[derive(Debug, thiserror::Error)]
pub enum DisconnectProjectError {
    /// No project matches the supplied id.
    #[error("project not found")]
    NotFound,
    /// Unexpected database failure.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Failure taxonomy for [`ProjectStore::reconnect_project`].
#[derive(Debug, thiserror::Error)]
pub enum ReconnectProjectError {
    /// No project matches the supplied id.
    #[error("project not found")]
    NotFound,
    /// Unexpected database failure.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Input for [`ProjectStore::reconnect_project_atomic`]. Carries the
/// pre-built lifecycle events so the store can append them and update
/// the projection in one transaction.
#[derive(Debug, Clone)]
pub struct ReconnectProjectAtomicRequest {
    /// Project to reconnect.
    pub project_id: ProjectId,
    /// Canonical lifecycle events to append before the projection write.
    pub events: Vec<serde_json::Value>,
    /// Wall-clock instant for event timestamps.
    pub now: DateTime<Utc>,
}

/// Successful return from [`ProjectStore::reconnect_project_atomic`].
#[derive(Debug, Clone)]
pub struct ReconnectProjectAtomicOutput {
    /// The reconnected project record (status is now `Connected`).
    pub project: ProjectRecord,
    /// Spec records retained from before the disconnection.
    pub specs: Vec<ProjectSpecRecord>,
}

/// Input for [`ProjectStore::connect_project_atomic`]. Bundles the
/// project row to insert with the lifecycle events to append so the
/// store can execute both as one transaction. Events are appended
/// *before* the projection row (audit-first semantics).
#[derive(Debug, Clone)]
pub struct ConnectProjectAtomicRequest {
    /// Project row to insert.
    pub project: NewProject,
    /// Canonical lifecycle events to append before the projection write.
    pub events: Vec<serde_json::Value>,
    /// Wall-clock instant for event timestamps and row writes.
    pub now: DateTime<Utc>,
}

/// Successful return from [`ProjectStore::connect_project_atomic`].
#[derive(Debug, Clone)]
pub struct ConnectProjectAtomicOutput {
    /// The inserted project record.
    pub project: ProjectRecord,
}

/// Input for [`ProjectStore::disconnect_project_atomic`]. Carries the
/// pre-built lifecycle events (unresolved dependency signals +
/// disconnection event) so the store can append them all and update
/// the projection in one transaction.
#[derive(Debug, Clone)]
pub struct DisconnectProjectAtomicRequest {
    /// Project to disconnect.
    pub project_id: ProjectId,
    /// Canonical lifecycle events to append before the projection write.
    pub events: Vec<serde_json::Value>,
    /// Wall-clock instant for event timestamps and the
    /// `disconnected_at` column.
    pub now: DateTime<Utc>,
}

/// Successful return from [`ProjectStore::disconnect_project_atomic`].
#[derive(Debug, Clone)]
pub struct DisconnectProjectAtomicOutput {
    /// The updated project record (status is now `Disconnected`).
    pub project: ProjectRecord,
}

/// Port for project lifecycle persistence. The SeaORM-backed adapter is
/// `impl ProjectStore for Store` (see `project_store.rs`).
///
/// Connect and disconnect are exposed *only* as atomic methods that
/// append lifecycle events before updating projection rows — there is
/// no public API to mutate `projects` rows without an accompanying
/// event.
#[async_trait]
pub trait ProjectStore: Send + Sync + std::fmt::Debug {
    /// Append the supplied lifecycle events, then insert the project
    /// row, all in one transaction. The event-first order guarantees
    /// the canonical event log captures the connection even if a
    /// transient failure rolls back the projection write.
    async fn connect_project_atomic(
        &self,
        request: ConnectProjectAtomicRequest,
    ) -> Result<ConnectProjectAtomicOutput, StoreError>;

    /// Find a project by org, provider connection, and resource, regardless
    /// of connection status. Returns `None` when no matching row exists.
    async fn find_project_by_org_and_resource(
        &self,
        org_id: OrgId,
        provider_connection_id: ProviderConnectionId,
        resource_id: &str,
    ) -> Result<Option<ProjectRecord>, StoreError>;

    /// Clear the disconnected state on an existing project and return
    /// the restored record alongside its retained spec history.
    async fn reconnect_project(
        &self,
        project_id: ProjectId,
    ) -> Result<ReconnectedProject, ReconnectProjectError>;

    /// Append the supplied lifecycle events, then clear
    /// `disconnected_at` on the project row, all in one transaction.
    /// Returns the updated project record and retained spec history.
    async fn reconnect_project_atomic(
        &self,
        request: ReconnectProjectAtomicRequest,
    ) -> Result<ReconnectProjectAtomicOutput, ReconnectProjectError>;

    /// List all connected (non-disconnected) projects visible to an
    /// account through its org memberships.
    async fn list_connected_projects_for_account(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<ProjectRecord>, StoreError>;

    /// Append the supplied lifecycle events, then set
    /// `disconnected_at` on the project row, all in one transaction.
    /// Does not delete any project, spec, dependency, or repository
    /// metadata rows.
    async fn disconnect_project_atomic(
        &self,
        request: DisconnectProjectAtomicRequest,
    ) -> Result<DisconnectProjectAtomicOutput, DisconnectProjectError>;

    /// Read all specs for a project, regardless of connection status.
    async fn read_project_specs(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<ProjectSpecRecord>, StoreError>;

    /// Read all dependency links originating from a project, annotating
    /// each with whether the target is resolved, disconnected, or
    /// unknown.
    async fn read_project_dependencies(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<ProjectDependencyLink>, StoreError>;

    /// Read all dependency links whose target is the supplied project,
    /// annotating each with whether the target is resolved, disconnected,
    /// or unknown. Used by disconnect to surface inbound unresolved-link
    /// signals from other projects that depend on the project being
    /// disconnected.
    async fn read_inbound_dependencies(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<ProjectDependencyLink>, StoreError>;

    /// Insert a loop-fixture row. M-0003 fixture seam for the
    /// no-active-loops precondition; replaced when M-0011 lands.
    async fn set_loop_fixture(
        &self,
        project_id: ProjectId,
        is_active: bool,
        now: DateTime<Utc>,
    ) -> Result<ProjectLoopFixtureRecord, StoreError>;

    /// Return `true` when at least one active loop fixture exists for
    /// the project.
    async fn has_active_loop_fixtures(&self, project_id: ProjectId) -> Result<bool, StoreError>;

    /// Return `true` when the account is a member of the org that owns
    /// the project. Used by the policy path to enforce project-level
    /// visibility for disconnect, reconnect, specs, and dependencies.
    async fn account_can_see_project(
        &self,
        account_id: AccountId,
        project_id: ProjectId,
    ) -> Result<bool, StoreError>;

    /// Return the org ids the account is a member of. Used by the
    /// policy path to construct [`ActorContext`] and to evaluate
    /// connect actions.
    async fn account_org_memberships(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<OrgId>, StoreError>;
}
