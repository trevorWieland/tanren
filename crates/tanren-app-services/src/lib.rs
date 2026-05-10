//! Command and query handlers shared by every Tanren interface binary.
//!
//! Per architecture, equivalent operations across web/api/cli/mcp/tui must
//! resolve to the same handler — this crate is that seam. Interface binaries
//! depend on `tanren-app-services` (and `tanren-contract` for wire shapes);
//! they do not import domain, store, or runtime crates directly.

pub mod account;
mod credential_sealer;
pub mod events;
mod handlers_user_configuration;
pub mod user_configuration;
mod user_configuration_pagination;
mod user_configuration_support;
use chrono::{DateTime, Utc};
use credential_sealer::CredentialSealerState;
use serde::{Deserialize, Serialize};
use tanren_configuration_secrets::{CredentialSealingFailure, CredentialValueSealer};
use tanren_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AccountFailureReason, ContractVersion,
    GetAuthenticatedAccountResponse, SignInRequest, SignInResponse, SignUpRequest, SignUpResponse,
    UserConfigurationFailureReason,
};
use tanren_identity_policy::{AccountId, Argon2idVerifier, CredentialVerifier, SessionToken};
pub use tanren_store::{AccountStore, Store};

use std::sync::Arc;
use tanren_store::{SessionAuthenticationLookup, StoreError};
use thiserror::Error;
pub use user_configuration_support::AuthenticatedConfigurationContext;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub status: &'static str,
    pub version: &'static str,
    pub contract_version: ContractVersion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionAuthenticationRequest {
    pub session_token: SessionToken,
    pub now: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedSessionView {
    pub authenticated_account_id: AccountId,
    pub expires_at: DateTime<Utc>,
}

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
    #[must_use]
    pub fn from_fn<F>(f: F) -> Self
    where
        F: Fn() -> DateTime<Utc> + Send + Sync + 'static,
    {
        Self { inner: Arc::new(f) }
    }

    #[must_use]
    pub fn now(&self) -> DateTime<Utc> {
        (self.inner)()
    }
}

#[derive(Debug, Clone)]
pub struct Handlers {
    clock: Clock,
    verifier: Arc<dyn CredentialVerifier>,
    value_sealing_state: CredentialSealerState,
}

impl Default for Handlers {
    fn default() -> Self {
        Self {
            clock: Clock::default(),
            verifier: Arc::new(Argon2idVerifier::production()),
            value_sealing_state: CredentialSealerState::from_env(),
        }
    }
}

impl Handlers {
    fn from_parts(
        clock: Clock,
        verifier: Arc<dyn CredentialVerifier>,
        value_sealing_state: CredentialSealerState,
    ) -> Self {
        Self {
            clock,
            verifier,
            value_sealing_state,
        }
    }

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_clock(clock: Clock) -> Self {
        Self::from_parts(
            clock,
            Arc::new(Argon2idVerifier::production()),
            CredentialSealerState::from_env(),
        )
    }

    #[must_use]
    pub fn with_verifier(clock: Clock, verifier: Arc<dyn CredentialVerifier>) -> Self {
        Self::from_parts(clock, verifier, CredentialSealerState::from_env())
    }

    #[must_use]
    pub fn with_credential_sealer_result(
        credential_sealer: Result<CredentialValueSealer, CredentialSealingFailure>,
    ) -> Self {
        Self::from_parts(
            Clock::default(),
            Arc::new(Argon2idVerifier::production()),
            CredentialSealerState::from_result(credential_sealer),
        )
    }

    #[must_use]
    pub fn with_verifier_and_credential_sealer_result(
        clock: Clock,
        verifier: Arc<dyn CredentialVerifier>,
        credential_sealer: Result<CredentialValueSealer, CredentialSealingFailure>,
    ) -> Self {
        Self::from_parts(
            clock,
            verifier,
            CredentialSealerState::from_result(credential_sealer),
        )
    }

    #[must_use]
    pub fn with_credential_sealer(credential_sealer: CredentialValueSealer) -> Self {
        Self::from_parts(
            Clock::default(),
            Arc::new(Argon2idVerifier::production()),
            CredentialSealerState::Ready(credential_sealer),
        )
    }

    #[must_use]
    pub fn with_verifier_and_credential_sealer(
        clock: Clock,
        verifier: Arc<dyn CredentialVerifier>,
        credential_sealer: CredentialValueSealer,
    ) -> Self {
        Self::from_parts(
            clock,
            verifier,
            CredentialSealerState::Ready(credential_sealer),
        )
    }

    #[must_use]
    pub fn health(&self, version: &'static str) -> HealthReport {
        HealthReport {
            status: "ok",
            version,
            contract_version: ContractVersion::CURRENT,
        }
    }

    pub async fn migrate(&self, database_url: &str) -> Result<(), AppServiceError> {
        let store = Store::connect(database_url).await?;
        store.migrate().await?;
        Ok(())
    }

    pub async fn authenticate_session<S>(
        &self,
        store: &S,
        request: SessionAuthenticationRequest,
    ) -> Result<Option<AuthenticatedSessionView>, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        let authenticated = store
            .authenticate_session(SessionAuthenticationLookup {
                session_token: request.session_token,
                now: request.now,
            })
            .await?;
        Ok(authenticated.map(|session| AuthenticatedSessionView {
            authenticated_account_id: session.authenticated_account_id,
            expires_at: session.expires_at,
        }))
    }

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

    pub async fn get_authenticated_account<S>(
        &self,
        store: &S,
        account_id: AccountId,
        session_expires_at: DateTime<Utc>,
    ) -> Result<Option<GetAuthenticatedAccountResponse>, AppServiceError>
    where
        S: AccountStore + ?Sized,
    {
        account::get_authenticated_account(store, &self.clock, account_id, session_expires_at).await
    }

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
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AppServiceError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("account: {}", .0.code())]
    Account(AccountFailureReason),
    #[error("configuration: {}", .0.code())]
    Configuration(UserConfigurationFailureReason),
}
