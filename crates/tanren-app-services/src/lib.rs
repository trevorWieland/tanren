//! Command and query handlers shared by every Tanren interface binary.
//!
//! Per architecture, equivalent operations across web/api/cli/mcp/tui must
//! resolve to the same handler — this crate is that seam. Interface binaries
//! depend on `tanren-app-services` (and `tanren-contract` for wire shapes);
//! they do not import domain, store, or runtime crates directly.

pub mod account;
pub mod events;
pub mod project;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tanren_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AccountFailureReason, ConnectProjectRequest,
    ConnectProjectResponse, ContractVersion, DisconnectProjectRequest, DisconnectProjectResponse,
    ListProjectsResponse, ProjectFailureReason, ReconnectProjectResponse, SignInRequest,
    SignInResponse, SignUpRequest, SignUpResponse,
};
use tanren_identity_policy::{Argon2idVerifier, CredentialVerifier, ProjectId};
pub use tanren_policy::ActorContext;
pub use tanren_provider_integrations::{
    LocalProviderRegistry, LocalSourceControlProvider, ProviderRegistry, SourceControlProvider,
    VoidProviderRegistry,
};
pub use tanren_store::{AccountStore, NewProject, ProjectStore, Store};

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

/// Stateless handler facade. Holds an injectable [`Clock`],
/// [`CredentialVerifier`], and [`ProviderRegistry`] so account and project
/// flow handlers stay deterministic — and cheaply hashed — under the BDD
/// harness.
#[derive(Debug, Clone)]
pub struct Handlers {
    clock: Clock,
    verifier: Arc<dyn CredentialVerifier>,
    providers: Arc<dyn ProviderRegistry>,
}

impl Default for Handlers {
    fn default() -> Self {
        let providers = Arc::new(LocalProviderRegistry::new());
        Self {
            clock: Clock::default(),
            verifier: Arc::new(Argon2idVerifier::production()),
            providers,
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
    /// production-strength [`Argon2idVerifier`] for hashing and a
    /// [`LocalProviderRegistry`] for source-control provider resolution.
    #[must_use]
    pub fn with_clock(clock: Clock) -> Self {
        let providers = Arc::new(LocalProviderRegistry::new());
        Self {
            clock,
            verifier: Arc::new(Argon2idVerifier::production()),
            providers,
        }
    }

    /// Construct a handler facade backed by an explicit
    /// [`CredentialVerifier`]. Production binaries that want to pin a
    /// non-default verifier (alternate parameter set, hardware-backed
    /// implementation) thread it in here.
    #[must_use]
    pub fn with_verifier(clock: Clock, verifier: Arc<dyn CredentialVerifier>) -> Self {
        let providers = Arc::new(LocalProviderRegistry::new());
        Self {
            clock,
            verifier,
            providers,
        }
    }

    /// Construct a handler facade with an explicit provider registry.
    /// Production binaries inject a real registry here; the BDD harness
    /// injects a [`LocalProviderRegistry`].
    #[must_use]
    pub fn with_providers(
        clock: Clock,
        verifier: Arc<dyn CredentialVerifier>,
        providers: Arc<dyn ProviderRegistry>,
    ) -> Self {
        Self {
            clock,
            verifier,
            providers,
        }
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

    pub async fn connect_project<S>(
        &self,
        store: &S,
        actor: &ActorContext,
        request: ConnectProjectRequest,
    ) -> Result<ConnectProjectResponse, AppServiceError>
    where
        S: AccountStore + ProjectStore + ?Sized,
    {
        project::connect_project(store, &self.clock, actor, self.providers.as_ref(), request).await
    }

    pub async fn list_projects<S>(
        &self,
        store: &S,
        actor: &ActorContext,
    ) -> Result<ListProjectsResponse, AppServiceError>
    where
        S: ProjectStore + ?Sized,
    {
        project::list_projects(store, actor).await
    }

    pub async fn disconnect_project<S>(
        &self,
        store: &S,
        actor: &ActorContext,
        request: DisconnectProjectRequest,
    ) -> Result<DisconnectProjectResponse, AppServiceError>
    where
        S: AccountStore + ProjectStore + ?Sized,
    {
        project::disconnect_project(store, &self.clock, actor, request).await
    }

    pub async fn project_specs<S>(
        &self,
        store: &S,
        actor: &ActorContext,
        project_id: ProjectId,
    ) -> Result<Vec<project::ProjectSpecView>, AppServiceError>
    where
        S: ProjectStore + ?Sized,
    {
        project::project_specs(store, actor, project_id).await
    }

    pub async fn project_dependencies<S>(
        &self,
        store: &S,
        actor: &ActorContext,
        project_id: ProjectId,
    ) -> Result<Vec<project::ProjectDependencyView>, AppServiceError>
    where
        S: ProjectStore + ?Sized,
    {
        project::project_dependencies(store, actor, project_id).await
    }

    pub async fn reconnect_project<S>(
        &self,
        store: &S,
        actor: &ActorContext,
        project_id: ProjectId,
    ) -> Result<ReconnectProjectResponse, AppServiceError>
    where
        S: ProjectStore + ?Sized,
    {
        project::reconnect_project(store, actor, project_id).await
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
    /// A project-flow taxonomy failure.
    #[error("project: {}", .0.code())]
    Project(ProjectFailureReason),
}
