//! Command and query handlers shared by every Tanren interface binary.
//!
//! Per architecture, equivalent operations across web/api/cli/mcp/tui must
//! resolve to the same handler — this crate is that seam. Interface binaries
//! depend on `tanren-app-services` (and `tanren-contract` for wire shapes);
//! they do not import domain, store, or runtime crates directly.

pub mod account;
pub mod events;
pub mod notifications;
pub mod user_configuration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tanren_configuration_secrets::{NotificationChannel, NotificationChannelSet};
use tanren_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AccountFailureReason,
    ConfigurationFailureReason, ContractVersion, CreateCredentialRequest, CreateCredentialResponse,
    EvaluateNotificationRouteRequest, EvaluateNotificationRouteResponse, GetUserConfigRequest,
    GetUserConfigResponse, ListCredentialsResponse, ListNotificationPreferencesResponse,
    ListUserConfigResponse, ReadPendingRoutingSnapshotResponse, RemoveCredentialRequest,
    RemoveCredentialResponse, RemoveUserConfigRequest, RemoveUserConfigResponse,
    SetNotificationPreferencesRequest, SetNotificationPreferencesResponse,
    SetOrganizationNotificationOverridesRequest, SetOrganizationNotificationOverridesResponse,
    SetUserConfigRequest, SetUserConfigResponse, SignInRequest, SignInResponse, SignUpRequest,
    SignUpResponse, UpdateCredentialRequest, UpdateCredentialResponse,
};
use tanren_identity_policy::{Argon2idVerifier, CredentialVerifier, SessionToken};
pub use tanren_store::{AccountStore, Store};

use std::sync::Arc;
use tanren_identity_policy::AccountId;
use tanren_store::StoreError;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct AuthenticatedActor {
    account_id: AccountId,
}

impl AuthenticatedActor {
    #[must_use]
    pub fn from_account_id(id: AccountId) -> Self {
        Self { account_id: id }
    }

    #[must_use]
    pub fn account_id(&self) -> AccountId {
        self.account_id
    }
}

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
    notification_supported_channels: NotificationChannelSet,
}

impl Default for Handlers {
    fn default() -> Self {
        Self {
            clock: Clock::default(),
            verifier: Arc::new(Argon2idVerifier::production()),
            notification_supported_channels: NotificationChannel::all().iter().copied().collect(),
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
            notification_supported_channels: NotificationChannel::all().iter().copied().collect(),
        }
    }

    /// Construct a handler facade backed by an explicit
    /// [`CredentialVerifier`]. Production binaries that want to pin a
    /// non-default verifier (alternate parameter set, hardware-backed
    /// implementation) thread it in here.
    #[must_use]
    pub fn with_verifier(clock: Clock, verifier: Arc<dyn CredentialVerifier>) -> Self {
        Self {
            clock,
            verifier,
            notification_supported_channels: NotificationChannel::all().iter().copied().collect(),
        }
    }

    #[must_use]
    pub fn with_notification_supported_channels(
        mut self,
        channels: NotificationChannelSet,
    ) -> Self {
        self.notification_supported_channels = channels;
        self
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

    pub async fn resolve_actor<S>(
        &self,
        store: &S,
        token: &SessionToken,
    ) -> Result<AuthenticatedActor, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        let now = self.clock.now();
        let account_id = store
            .find_account_id_by_session_token(token, now)
            .await?
            .ok_or_else(|| AppServiceError::InvalidInput("invalid or expired session".into()))?;
        Ok(AuthenticatedActor::from_account_id(account_id))
    }

    pub async fn list_user_config<S>(
        &self,
        store: &S,
        actor: &AuthenticatedActor,
    ) -> Result<ListUserConfigResponse, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        user_configuration::list_user_config(store, actor).await
    }

    pub async fn get_user_config<S>(
        &self,
        store: &S,
        actor: &AuthenticatedActor,
        request: GetUserConfigRequest,
    ) -> Result<GetUserConfigResponse, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        user_configuration::get_user_config(store, actor, request).await
    }

    pub async fn set_user_config<S>(
        &self,
        store: &S,
        actor: &AuthenticatedActor,
        request: SetUserConfigRequest,
    ) -> Result<SetUserConfigResponse, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        user_configuration::set_user_config(store, &self.clock, actor, request).await
    }

    pub async fn remove_user_config<S>(
        &self,
        store: &S,
        actor: &AuthenticatedActor,
        request: RemoveUserConfigRequest,
    ) -> Result<RemoveUserConfigResponse, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        user_configuration::remove_user_config(store, &self.clock, actor, request).await
    }

    pub async fn list_credentials<S>(
        &self,
        store: &S,
        actor: &AuthenticatedActor,
    ) -> Result<ListCredentialsResponse, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        user_configuration::list_credentials(store, actor).await
    }

    pub async fn create_credential<S>(
        &self,
        store: &S,
        actor: &AuthenticatedActor,
        request: CreateCredentialRequest,
    ) -> Result<CreateCredentialResponse, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        user_configuration::create_credential(store, &self.clock, actor, request).await
    }

    pub async fn update_credential<S>(
        &self,
        store: &S,
        actor: &AuthenticatedActor,
        request: UpdateCredentialRequest,
    ) -> Result<UpdateCredentialResponse, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        user_configuration::update_credential(store, &self.clock, actor, request).await
    }

    pub async fn remove_credential<S>(
        &self,
        store: &S,
        actor: &AuthenticatedActor,
        request: RemoveCredentialRequest,
    ) -> Result<RemoveCredentialResponse, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        user_configuration::remove_credential(store, &self.clock, actor, request).await
    }

    pub async fn set_notification_preferences<S>(
        &self,
        store: &S,
        actor: &AuthenticatedActor,
        request: SetNotificationPreferencesRequest,
    ) -> Result<SetNotificationPreferencesResponse, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        notifications::set_notification_preferences(
            store,
            &self.clock,
            actor,
            &self.notification_supported_channels,
            request,
        )
        .await
    }

    pub async fn list_notification_preferences<S>(
        &self,
        store: &S,
        actor: &AuthenticatedActor,
    ) -> Result<ListNotificationPreferencesResponse, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        notifications::list_notification_preferences(store, actor).await
    }

    pub async fn set_organization_notification_overrides<S>(
        &self,
        store: &S,
        actor: &AuthenticatedActor,
        request: SetOrganizationNotificationOverridesRequest,
    ) -> Result<SetOrganizationNotificationOverridesResponse, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        notifications::set_organization_notification_overrides(
            store,
            &self.clock,
            actor,
            &self.notification_supported_channels,
            request,
        )
        .await
    }

    pub async fn evaluate_notification_route<S>(
        &self,
        store: &S,
        actor: &AuthenticatedActor,
        request: EvaluateNotificationRouteRequest,
    ) -> Result<EvaluateNotificationRouteResponse, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        notifications::evaluate_notification_route(store, &self.clock, actor, request).await
    }

    pub async fn read_pending_routing_snapshot<S>(
        &self,
        store: &S,
        actor: &AuthenticatedActor,
    ) -> Result<ReadPendingRoutingSnapshotResponse, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        notifications::read_pending_routing_snapshot(store, &self.clock, actor).await
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
    /// A configuration or credential-flow taxonomy failure.
    #[error("configuration: {}", .0.code())]
    Configuration(ConfigurationFailureReason),
}
