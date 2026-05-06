//! Command and query handlers shared by every Tanren interface binary.
//!
//! Per architecture, equivalent operations across web/api/cli/mcp/tui must
//! resolve to the same handler — this crate is that seam. Interface binaries
//! depend on `tanren-app-services` (and `tanren-contract` for wire shapes);
//! they do not import domain, store, or runtime crates directly.

pub mod account;
pub mod events;
pub mod join;
pub mod membership_departure;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tanren_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AccountFailureReason, ContractVersion,
    JoinOrganizationRequest, JoinOrganizationResponse, LeaveOrganizationRequest,
    MembershipDepartureResponse, RemoveMemberRequest, SignInRequest, SignInResponse, SignUpRequest,
    SignUpResponse,
};
use tanren_identity_policy::{AccountId, Argon2idVerifier, CredentialVerifier};
pub use tanren_store::{AccountStore, Store};

use std::sync::Arc;
use tanren_store::StoreError;
use thiserror::Error;

/// Stable response shape for the cross-interface health/liveness query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    /// Static "ok" string. Present so consumers can match on a discriminator
    /// rather than HTTP status alone.
    pub status: &'static str,
    /// Build-time package version of the binary that produced the report.
    pub version: &'static str,
    /// Wire-contract version this binary speaks.
    pub contract_version: ContractVersion,
}

/// Injected wall-clock. BDD scenarios swap this for a deterministic
/// fake; production binaries keep [`Clock::default`] (reads
/// `chrono::Utc::now()`).
#[derive(Clone)]
pub struct Clock {
    inner: Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>,
}

impl std::fmt::Debug for Clock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Clock").finish_non_exhaustive()
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            inner: Arc::new(Utc::now),
        }
    }
}

impl Clock {
    /// Wrap a custom `now` impl. The BDD harness uses this to make
    /// invitation-expiry scenarios deterministic.
    #[must_use]
    pub fn from_fn<F>(f: F) -> Self
    where
        F: Fn() -> DateTime<Utc> + Send + Sync + 'static,
    {
        Self { inner: Arc::new(f) }
    }

    /// Current wall-clock instant according to this `Clock`.
    #[must_use]
    pub fn now(&self) -> DateTime<Utc> {
        (self.inner)()
    }
}

/// Stateless handler facade. Holds an injectable [`Clock`] and
/// [`CredentialVerifier`] so account flow handlers stay deterministic —
/// and cheaply hashed — under the BDD harness.
#[derive(Debug, Clone)]
pub struct Handlers {
    clock: Clock,
    verifier: Arc<dyn CredentialVerifier>,
}

impl Default for Handlers {
    fn default() -> Self {
        Self {
            clock: Clock::default(),
            verifier: Arc::new(Argon2idVerifier::production()),
        }
    }
}

impl Handlers {
    /// Construct a handler facade backed by [`Clock::default`] and the
    /// production-strength [`Argon2idVerifier`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct a handler facade backed by an explicit clock. Uses the
    /// production-strength [`Argon2idVerifier`] for hashing.
    #[must_use]
    pub fn with_clock(clock: Clock) -> Self {
        Self {
            clock,
            verifier: Arc::new(Argon2idVerifier::production()),
        }
    }

    /// Construct a handler facade backed by an explicit
    /// [`CredentialVerifier`]. Production binaries that want to pin a
    /// non-default verifier (alternate parameter set, hardware-backed
    /// implementation) thread it in here.
    #[must_use]
    pub fn with_verifier(clock: Clock, verifier: Arc<dyn CredentialVerifier>) -> Self {
        Self { clock, verifier }
    }

    /// Liveness query. Returns the same shape regardless of which interface
    /// invoked it.
    #[must_use]
    pub fn health(&self, version: &'static str) -> HealthReport {
        HealthReport {
            status: "ok",
            version,
            contract_version: ContractVersion::CURRENT,
        }
    }

    /// Apply all pending database migrations against the supplied URL.
    ///
    /// # Errors
    ///
    /// Returns [`AppServiceError::Store`] if connection or migration fails.
    pub async fn migrate(&self, database_url: &str) -> Result<(), AppServiceError> {
        let store = Store::connect(database_url).await?;
        store.migrate().await?;
        Ok(())
    }

    /// Self-signup command: create a new personal account, mint a
    /// session, and append an `account_created` event.
    ///
    /// # Errors
    ///
    /// Returns [`AppServiceError::Account`] for taxonomy failures
    /// (duplicate identifier, invalid credential), or
    /// [`AppServiceError::Store`] for unexpected database failures.
    pub async fn sign_up<S>(
        &self,
        store: &S,
        request: SignUpRequest,
    ) -> Result<SignUpResponse, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        account::sign_up(store, &self.clock, self.verifier.as_ref(), request).await
    }

    /// Sign-in command: verify an identifier+password against the
    /// stored hash and mint a fresh session.
    ///
    /// # Errors
    ///
    /// Returns [`AppServiceError::Account`] with
    /// [`AccountFailureReason::InvalidCredential`] when the credential
    /// does not verify; [`AppServiceError::Store`] for unexpected
    /// database failures.
    pub async fn sign_in<S>(
        &self,
        store: &S,
        request: SignInRequest,
    ) -> Result<SignInResponse, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        account::sign_in(store, &self.clock, self.verifier.as_ref(), request).await
    }

    /// Invitation-acceptance command: consume the supplied token,
    /// create an account joined to the inviting org, and append both
    /// `account_created` and `invitation_accepted` events.
    ///
    /// # Errors
    ///
    /// Returns [`AppServiceError::Account`] with the matching
    /// invitation taxonomy variant when the token is unknown / expired
    /// / already consumed; [`AppServiceError::Store`] for unexpected
    /// database failures.
    pub async fn accept_invitation<S>(
        &self,
        store: &S,
        request: AcceptInvitationRequest,
    ) -> Result<AcceptInvitationResponse, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        account::accept_invitation(store, &self.clock, self.verifier.as_ref(), request).await
    }

    /// Existing-account join command: an authenticated account accepts an
    /// invitation to join an organization. Unlike [`Handlers::accept_invitation`]
    /// (which creates a new account), this flow operates on an existing
    /// account identified by `account_id`. On success the account gains a
    /// membership in the inviting organization carrying the invitation's
    /// org-level permissions; the account's other memberships are
    /// unaffected. Project access is NOT granted automatically.
    ///
    /// # Errors
    ///
    /// Returns [`AppServiceError::Account`] with the matching taxonomy
    /// variant when the token is unknown / expired / already consumed /
    /// addressed to a different account, or when the account is already a
    /// member; [`AppServiceError::Store`] for unexpected database failures.
    pub async fn join_organization<S>(
        &self,
        store: &S,
        account_id: AccountId,
        request: JoinOrganizationRequest,
    ) -> Result<JoinOrganizationResponse, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        join::join_organization(store, &self.clock, account_id, request).await
    }

    /// Voluntary leave command: an authenticated member leaves an
    /// organization. The member's other organization memberships are
    /// unaffected. In-flight work is surfaced when
    /// `acknowledge_in_flight_work` is `false` and the response
    /// indicates preview-before-completion; setting the flag to
    /// `true` completes the departure.
    ///
    /// # Errors
    ///
    /// Returns [`AppServiceError::Account`] with
    /// [`AccountFailureReason::NotOrgMember`] when the caller is not
    /// a member; [`AccountFailureReason::LastAdminPermissionHolder`]
    /// when the caller is the sole admin; [`AppServiceError::Store`]
    /// for unexpected database failures.
    pub async fn leave_organization<S>(
        &self,
        store: &S,
        account_id: AccountId,
        request: LeaveOrganizationRequest,
        acknowledge_in_flight_work: bool,
    ) -> Result<MembershipDepartureResponse, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        membership_departure::leave_organization(
            store,
            &self.clock,
            account_id,
            request,
            acknowledge_in_flight_work,
        )
        .await
    }

    /// Admin-initiated member removal command: an authenticated admin
    /// removes another account from an organization. The target
    /// account and its other memberships are preserved. In-flight
    /// work is surfaced when `acknowledge_in_flight_work` is `false`
    /// and the response indicates preview-before-completion; setting
    /// the flag to `true` completes the departure.
    ///
    /// # Errors
    ///
    /// Returns [`AppServiceError::Account`] with
    /// [`AccountFailureReason::PermissionDenied`] when the caller is
    /// not an admin; [`AccountFailureReason::NotOrgMember`] when the
    /// target is not a member;
    /// [`AccountFailureReason::LastAdminPermissionHolder`] when the
    /// target is the sole admin; [`AppServiceError::Store`] for
    /// unexpected database failures.
    pub async fn remove_member<S>(
        &self,
        store: &S,
        actor_account_id: AccountId,
        request: RemoveMemberRequest,
        acknowledge_in_flight_work: bool,
    ) -> Result<MembershipDepartureResponse, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        membership_departure::remove_member(
            store,
            &self.clock,
            actor_account_id,
            request,
            acknowledge_in_flight_work,
        )
        .await
    }
}

/// Errors raised by app-service handlers.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AppServiceError {
    /// A handler input failed validation.
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// The underlying store layer raised an error.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// A taxonomy failure interface binaries map to a `{code, summary}`
    /// error body.
    #[error("account: {}", .0.code())]
    Account(AccountFailureReason),
}
