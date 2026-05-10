//! Command and query handlers shared by every Tanren interface binary.
//!
//! Per architecture, equivalent operations across web/api/cli/mcp/tui must
//! resolve to the same handler — this crate is that seam. Interface binaries
//! depend on `tanren-app-services` (and `tanren-contract` for wire shapes);
//! they do not import domain, store, or runtime crates directly.

pub mod account;
pub mod events;
pub mod mcp_auth;
pub mod organization;
pub mod organization_errors;

pub use crate::organization_errors::{OrganizationErrorProjection, map_organization_error};
use chrono::{DateTime, Utc};
pub use mcp_auth::{
    MCP_API_KEY_ENV, McpActorContext, McpAuthConfig, McpAuthConfigError, McpAuthFailure,
};
use serde::{Deserialize, Serialize};
use tanren_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AccountFailureReason,
    CheckOrganizationPermissionRequest, CheckOrganizationPermissionResponse, ContractVersion,
    CreateOrganizationFailureReason, CreateOrganizationRequest, CreateOrganizationResponse,
    ListOrganizationsRequest, ListOrganizationsResponse, SignInRequest, SignInResponse,
    SignUpRequest, SignUpResponse,
};
use tanren_identity_policy::{Argon2idVerifier, CredentialVerifier};
use tanren_identity_policy::{OrganizationCapability, OrganizationPermissionGate};
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

    /// Create an organization for a currently authenticated account.
    ///
    /// # Errors
    ///
    /// Returns [`AppServiceError::Account`] with
    /// [`AccountFailureReason::AuthRequired`] when authentication is
    /// missing/expired. Returns
    /// [`AppServiceError::CreateOrganization`] for duplicate-name and
    /// idempotency-conflict failures. Returns [`AppServiceError::Store`]
    /// for unexpected store failures.
    pub async fn create_organization<S>(
        &self,
        store: &S,
        request: CreateOrganizationRequest,
    ) -> Result<CreateOrganizationResponse, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        organization::create_organization(store, &self.clock, request).await
    }

    /// List organizations currently available to the authenticated
    /// account.
    ///
    /// # Errors
    ///
    /// Returns [`AppServiceError::Account`] with
    /// [`AccountFailureReason::AuthRequired`] when authentication is
    /// missing/expired. Returns [`AppServiceError::Store`] for
    /// unexpected store failures.
    pub async fn list_organizations<S>(
        &self,
        store: &S,
        request: ListOrganizationsRequest,
    ) -> Result<ListOrganizationsResponse, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        organization::list_organizations(store, &self.clock, request).await
    }

    /// Check whether an authenticated account currently holds a given
    /// organization permission.
    ///
    /// # Errors
    ///
    /// Returns [`AppServiceError::Account`] with
    /// [`AccountFailureReason::AuthRequired`] when authentication is
    /// missing/expired. Returns [`AppServiceError::Store`] for
    /// unexpected store failures.
    pub async fn check_organization_permission<S>(
        &self,
        store: &S,
        request: CheckOrganizationPermissionRequest,
    ) -> Result<CheckOrganizationPermissionResponse, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        organization::check_organization_permission(store, &self.clock, request).await
    }

    /// Resolve capability metadata from an organization permission.
    #[must_use]
    pub const fn organization_permission_capability(
        permission: tanren_identity_policy::OrganizationPermission,
    ) -> OrganizationCapability {
        tanren_identity_policy::organization_capability(permission)
    }

    /// Generic identity-policy-owned organization capability guard.
    ///
    /// # Errors
    ///
    /// Returns [`AppServiceError::Account`] with
    /// [`AccountFailureReason::AuthRequired`] when authentication is
    /// missing/expired or [`AccountFailureReason::PermissionDenied`]
    /// when the caller lacks the requested capability.
    pub async fn ensure_organization_capability<S>(
        &self,
        store: &S,
        session_token: &tanren_identity_policy::SessionToken,
        account_id: tanren_identity_policy::AccountId,
        org_id: tanren_identity_policy::OrgId,
        gate: OrganizationPermissionGate,
    ) -> Result<OrganizationCapability, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        let granted_gate = organization::require_organization_permission_gate(
            store,
            &self.clock,
            session_token,
            account_id,
            org_id,
            gate,
        )
        .await?;
        Ok(granted_gate.capability)
    }

    /// Permission guard for invitation issuance flows.
    ///
    /// # Errors
    ///
    /// Returns [`AppServiceError::Account`] with
    /// [`AccountFailureReason::AuthRequired`] when authentication is
    /// missing/expired or [`AccountFailureReason::PermissionDenied`]
    /// when the caller lacks invite rights.
    pub async fn ensure_can_invite_to_organization<S>(
        &self,
        store: &S,
        session_token: &tanren_identity_policy::SessionToken,
        account_id: tanren_identity_policy::AccountId,
        org_id: tanren_identity_policy::OrgId,
    ) -> Result<OrganizationCapability, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        self.ensure_organization_capability(
            store,
            session_token,
            account_id,
            org_id,
            OrganizationPermissionGate::from_permission(
                tanren_identity_policy::OrganizationPermission::Invite,
            ),
        )
        .await
    }

    /// Permission guard for member-removal and access-management flows.
    ///
    /// # Errors
    ///
    /// Returns [`AppServiceError::Account`] with
    /// [`AccountFailureReason::AuthRequired`] when authentication is
    /// missing/expired or [`AccountFailureReason::PermissionDenied`]
    /// when the caller lacks access-management rights.
    pub async fn ensure_can_manage_organization_access<S>(
        &self,
        store: &S,
        session_token: &tanren_identity_policy::SessionToken,
        account_id: tanren_identity_policy::AccountId,
        org_id: tanren_identity_policy::OrgId,
    ) -> Result<OrganizationCapability, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        self.ensure_organization_capability(
            store,
            session_token,
            account_id,
            org_id,
            OrganizationPermissionGate::from_permission(
                tanren_identity_policy::OrganizationPermission::ManageAccess,
            ),
        )
        .await
    }

    /// Permission guard for organization-configuration flows.
    ///
    /// # Errors
    ///
    /// Returns [`AppServiceError::Account`] with
    /// [`AccountFailureReason::AuthRequired`] when authentication is
    /// missing/expired or [`AccountFailureReason::PermissionDenied`]
    /// when the caller lacks configuration rights.
    pub async fn ensure_can_configure_organization<S>(
        &self,
        store: &S,
        session_token: &tanren_identity_policy::SessionToken,
        account_id: tanren_identity_policy::AccountId,
        org_id: tanren_identity_policy::OrgId,
    ) -> Result<OrganizationCapability, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        self.ensure_organization_capability(
            store,
            session_token,
            account_id,
            org_id,
            OrganizationPermissionGate::from_permission(
                tanren_identity_policy::OrganizationPermission::Configure,
            ),
        )
        .await
    }

    /// Permission guard for organization-policy editing flows.
    ///
    /// # Errors
    ///
    /// Returns [`AppServiceError::Account`] with
    /// [`AccountFailureReason::AuthRequired`] when authentication is
    /// missing/expired or [`AccountFailureReason::PermissionDenied`]
    /// when the caller lacks policy-management rights.
    pub async fn ensure_can_set_organization_policy<S>(
        &self,
        store: &S,
        session_token: &tanren_identity_policy::SessionToken,
        account_id: tanren_identity_policy::AccountId,
        org_id: tanren_identity_policy::OrgId,
    ) -> Result<OrganizationCapability, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        self.ensure_organization_capability(
            store,
            session_token,
            account_id,
            org_id,
            OrganizationPermissionGate::from_permission(
                tanren_identity_policy::OrganizationPermission::SetPolicy,
            ),
        )
        .await
    }

    /// Permission guard for organization deletion flows.
    ///
    /// # Errors
    ///
    /// Returns [`AppServiceError::Account`] with
    /// [`AccountFailureReason::AuthRequired`] when authentication is
    /// missing/expired or [`AccountFailureReason::PermissionDenied`]
    /// when the caller lacks delete rights.
    pub async fn ensure_can_delete_organization<S>(
        &self,
        store: &S,
        session_token: &tanren_identity_policy::SessionToken,
        account_id: tanren_identity_policy::AccountId,
        org_id: tanren_identity_policy::OrgId,
    ) -> Result<OrganizationCapability, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        self.ensure_organization_capability(
            store,
            session_token,
            account_id,
            org_id,
            OrganizationPermissionGate::from_permission(
                tanren_identity_policy::OrganizationPermission::Delete,
            ),
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
    /// Typed organization-create failure reason.
    #[error("organization_create: {}", .0.code())]
    CreateOrganization(CreateOrganizationFailureReason),
}
