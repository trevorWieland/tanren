//! Command and query handlers shared by every Tanren interface binary.
//!
//! Per architecture, equivalent operations across web/api/cli/mcp/tui must
//! resolve to the same handler — this crate is that seam. Interface binaries
//! depend on `tanren-app-services` (and `tanren-contract` for wire shapes);
//! they do not import domain, store, or runtime crates directly.

pub mod account;
pub mod events;
pub mod organization;
pub mod organization_events;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tanren_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AccountFailureReason, ContractVersion,
    CreateOrganizationRequest, ListOrganizationsRequest, ListOrganizationsResponse,
    OrganizationAdminOperation, OrganizationFailureReason, SignInRequest, SignInResponse,
    SignUpRequest, SignUpResponse,
};
use tanren_identity_policy::{
    AccountId, Argon2idVerifier, CredentialVerifier, OrgId, OrgPermission,
};
pub use tanren_store::{AccountStore, OrganizationStore, Store};

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

    /// Organization-creation command for a known account: create the
    /// org, link a membership, grant all five bootstrap admin
    /// permissions, and append an `organization_created` event.
    ///
    /// # Errors
    ///
    /// Returns [`AppServiceError::Organization`] with
    /// [`OrganizationFailureReason::DuplicateOrganizationName`] when
    /// the canonical name is already taken, or
    /// [`AppServiceError::Store`] for unexpected database failures.
    pub async fn create_organization_for_account<S>(
        &self,
        store: &S,
        account_id: AccountId,
        request: CreateOrganizationRequest,
    ) -> Result<organization::CreateOrganizationOutput, AppServiceError>
    where
        S: AccountStore + OrganizationStore + ?Sized,
    {
        organization::create_organization_for_account(store, &self.clock, account_id, request).await
    }

    /// Session-backed organization-creation command: resolve the
    /// bearer token to an account, then delegate to
    /// [`Handlers::create_organization_for_account`].
    ///
    /// # Errors
    ///
    /// Returns [`AppServiceError::Organization`] with
    /// [`OrganizationFailureReason::AuthRequired`] when the session is
    /// missing or expired; delegates all other failure modes to the
    /// account-based variant.
    pub async fn create_organization_with_session<S>(
        &self,
        store: &S,
        bearer_token: &str,
        request: CreateOrganizationRequest,
    ) -> Result<organization::CreateOrganizationOutput, AppServiceError>
    where
        S: AccountStore + OrganizationStore + ?Sized,
    {
        organization::create_organization_with_session(store, &self.clock, bearer_token, request)
            .await
    }

    /// Session-backed organization listing: resolve the bearer token to
    /// an account, then delegate to [`Handlers::list_account_organizations`].
    ///
    /// # Errors
    ///
    /// Returns [`AppServiceError::Organization`] with
    /// [`OrganizationFailureReason::AuthRequired`] when the session is
    /// missing or expired; delegates all other failure modes to the
    /// account-based variant.
    pub async fn list_account_organizations_with_session<S>(
        &self,
        store: &S,
        bearer_token: &str,
        request: ListOrganizationsRequest,
    ) -> Result<ListOrganizationsResponse, AppServiceError>
    where
        S: AccountStore + OrganizationStore + ?Sized,
    {
        organization::list_account_organizations_with_session(
            store,
            &self.clock,
            bearer_token,
            request,
        )
        .await
    }

    /// List organizations the supplied account is a member of, with
    /// bounded pagination.
    ///
    /// # Errors
    ///
    /// Returns [`AppServiceError::Store`] for unexpected database
    /// failures.
    pub async fn list_account_organizations<S>(
        &self,
        store: &S,
        account_id: AccountId,
        request: ListOrganizationsRequest,
    ) -> Result<ListOrganizationsResponse, AppServiceError>
    where
        S: OrganizationStore + ?Sized,
    {
        organization::list_account_organizations(store, account_id, request).await
    }

    /// Session-backed admin authorization probe: resolve the bearer
    /// token to an account, then delegate to
    /// [`Handlers::authorize_org_admin_operation`].
    ///
    /// # Errors
    ///
    /// Returns [`AppServiceError::Organization`] with
    /// [`OrganizationFailureReason::AuthRequired`] when the session is
    /// missing or expired; delegates all other failure modes to the
    /// account-based variant.
    pub async fn authorize_org_admin_operation_with_session<S>(
        &self,
        store: &S,
        bearer_token: &str,
        org_id: OrgId,
        operation: OrganizationAdminOperation,
    ) -> Result<(), AppServiceError>
    where
        S: AccountStore + OrganizationStore + ?Sized,
    {
        organization::authorize_org_admin_operation_with_session(
            store,
            &self.clock,
            bearer_token,
            org_id,
            operation,
        )
        .await
    }

    /// No-op authorization probe: returns `Ok(())` when the account
    /// holds the permission matching the requested admin operation on
    /// the specified organization.
    ///
    /// This is a read-only check — it does not mutate any state.
    ///
    /// # Errors
    ///
    /// Returns [`AppServiceError::Organization`] with
    /// [`OrganizationFailureReason::PermissionDenied`] when the
    /// account lacks the requested permission or is not a member of
    /// the organization.
    pub async fn authorize_org_admin_operation<S>(
        &self,
        store: &S,
        account_id: AccountId,
        org_id: OrgId,
        operation: OrganizationAdminOperation,
    ) -> Result<(), AppServiceError>
    where
        S: OrganizationStore + ?Sized,
    {
        organization::authorize_org_admin_operation(store, account_id, org_id, operation).await
    }

    /// Enforce the last-admin-holder invariant: returns
    /// `Err(LastAdminHolder)` when the account is the sole holder of
    /// the specified permission in the organization.
    ///
    /// This is a read-only check intended for leave/remove flows
    /// (R-0007) to call before revoking a permission or removing a
    /// member.
    ///
    /// # Errors
    ///
    /// Returns [`AppServiceError::Organization`] with
    /// [`OrganizationFailureReason::LastAdminHolder`] when revoking
    /// the permission would leave the organization without any holder.
    pub async fn assert_not_last_admin_holder<S>(
        &self,
        store: &S,
        org_id: OrgId,
        account_id: AccountId,
        permission: OrgPermission,
    ) -> Result<(), AppServiceError>
    where
        S: OrganizationStore + ?Sized,
    {
        organization::assert_not_last_admin_holder(store, org_id, account_id, permission).await
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
    /// An organization-flow taxonomy failure.
    #[error("organization: {}", .0.code())]
    Organization(OrganizationFailureReason),
}
