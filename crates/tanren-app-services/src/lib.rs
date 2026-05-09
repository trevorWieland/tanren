//! Command and query handlers shared by every Tanren interface binary.
//!
//! Per architecture, equivalent operations across web/api/cli/mcp/tui must
//! resolve to the same handler — this crate is that seam. Interface binaries
//! depend on `tanren-app-services` (and `tanren-contract` for wire shapes);
//! they do not import domain, store, or runtime crates directly.

pub mod account;
pub mod events;
pub mod user_configuration;
mod user_configuration_pagination;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tanren_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AccountFailureReason, ContractVersion,
    CreateUserCredentialRequest, CreateUserCredentialResponse, ListUserCredentialsRequest,
    ListUserCredentialsResponse, ListUserSettingsRequest, ListUserSettingsResponse,
    RemoveUserCredentialResponse, RemoveUserSettingResponse, SignInRequest, SignInResponse,
    SignUpRequest, SignUpResponse, UpdateUserCredentialRequest, UpdateUserCredentialResponse,
    UpsertUserSettingRequest, UpsertUserSettingResponse, UserConfigurationFailureReason,
};
use tanren_identity_policy::{AccountId, Argon2idVerifier, CredentialVerifier};
use tanren_store::UserConfigurationStore;
pub use tanren_store::{AccountStore, Store};

use std::sync::Arc;
use tanren_store::StoreError;
use thiserror::Error;
pub use user_configuration::AuthenticatedConfigurationContext;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub status: &'static str,
    pub version: &'static str,
    pub contract_version: ContractVersion,
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
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_clock(clock: Clock) -> Self {
        Self {
            clock,
            verifier: Arc::new(Argon2idVerifier::production()),
        }
    }

    #[must_use]
    pub fn with_verifier(clock: Clock, verifier: Arc<dyn CredentialVerifier>) -> Self {
        Self { clock, verifier }
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

    pub async fn list_user_settings<S>(
        &self,
        store: &S,
        authenticated_account_id: AccountId,
        requested_account_id: AccountId,
    ) -> Result<ListUserSettingsResponse, AppServiceError>
    where
        S: UserConfigurationStore + ?Sized,
    {
        self.list_user_settings_with_context(
            store,
            AuthenticatedConfigurationContext::for_requested_account(
                authenticated_account_id,
                requested_account_id,
            ),
            ListUserSettingsRequest::default(),
        )
        .await
    }

    pub async fn list_user_settings_with_context<S>(
        &self,
        store: &S,
        context: AuthenticatedConfigurationContext,
        request: ListUserSettingsRequest,
    ) -> Result<ListUserSettingsResponse, AppServiceError>
    where
        S: UserConfigurationStore + ?Sized,
    {
        user_configuration::list_user_settings(store, context, request).await
    }

    pub async fn list_user_settings_page<S>(
        &self,
        store: &S,
        authenticated_account_id: AccountId,
        requested_account_id: AccountId,
        request: ListUserSettingsRequest,
    ) -> Result<ListUserSettingsResponse, AppServiceError>
    where
        S: UserConfigurationStore + ?Sized,
    {
        self.list_user_settings_with_context(
            store,
            AuthenticatedConfigurationContext::for_requested_account(
                authenticated_account_id,
                requested_account_id,
            ),
            request,
        )
        .await
    }

    pub async fn upsert_user_setting<S>(
        &self,
        store: &S,
        authenticated_account_id: AccountId,
        requested_account_id: AccountId,
        request: UpsertUserSettingRequest,
    ) -> Result<UpsertUserSettingResponse, AppServiceError>
    where
        S: UserConfigurationStore + AccountStore + ?Sized,
    {
        self.upsert_user_setting_with_context(
            store,
            AuthenticatedConfigurationContext::for_requested_account(
                authenticated_account_id,
                requested_account_id,
            ),
            request,
        )
        .await
    }

    pub async fn upsert_user_setting_with_context<S>(
        &self,
        store: &S,
        context: AuthenticatedConfigurationContext,
        request: UpsertUserSettingRequest,
    ) -> Result<UpsertUserSettingResponse, AppServiceError>
    where
        S: UserConfigurationStore + AccountStore + ?Sized,
    {
        user_configuration::upsert_user_setting(store, &self.clock, context, request).await
    }

    pub async fn remove_user_setting<S>(
        &self,
        store: &S,
        authenticated_account_id: AccountId,
        requested_account_id: AccountId,
        key: tanren_configuration_secrets::UserSettingKey,
    ) -> Result<RemoveUserSettingResponse, AppServiceError>
    where
        S: UserConfigurationStore + AccountStore + ?Sized,
    {
        self.remove_user_setting_with_context(
            store,
            AuthenticatedConfigurationContext::for_requested_account(
                authenticated_account_id,
                requested_account_id,
            ),
            key,
        )
        .await
    }

    pub async fn remove_user_setting_with_context<S>(
        &self,
        store: &S,
        context: AuthenticatedConfigurationContext,
        key: tanren_configuration_secrets::UserSettingKey,
    ) -> Result<RemoveUserSettingResponse, AppServiceError>
    where
        S: UserConfigurationStore + AccountStore + ?Sized,
    {
        user_configuration::remove_user_setting(store, &self.clock, context, key).await
    }

    pub async fn add_user_credential<S>(
        &self,
        store: &S,
        authenticated_account_id: AccountId,
        request: CreateUserCredentialRequest,
    ) -> Result<CreateUserCredentialResponse, AppServiceError>
    where
        S: UserConfigurationStore + AccountStore + ?Sized,
    {
        self.add_user_credential_with_context(
            store,
            AuthenticatedConfigurationContext::for_requested_owner_scope(
                authenticated_account_id,
                request.owner_scope,
            ),
            request,
        )
        .await
    }

    pub async fn add_user_credential_with_context<S>(
        &self,
        store: &S,
        context: AuthenticatedConfigurationContext,
        request: CreateUserCredentialRequest,
    ) -> Result<CreateUserCredentialResponse, AppServiceError>
    where
        S: UserConfigurationStore + AccountStore + ?Sized,
    {
        user_configuration::add_user_credential(store, &self.clock, context, request).await
    }

    pub async fn update_user_credential<S>(
        &self,
        store: &S,
        authenticated_account_id: AccountId,
        item_id: tanren_configuration_secrets::UserCredentialId,
        owner_scope: tanren_configuration_secrets::OwnerScope,
        request: UpdateUserCredentialRequest,
    ) -> Result<UpdateUserCredentialResponse, AppServiceError>
    where
        S: UserConfigurationStore + AccountStore + ?Sized,
    {
        self.update_user_credential_with_context(
            store,
            AuthenticatedConfigurationContext::for_requested_owner_scope(
                authenticated_account_id,
                owner_scope,
            ),
            item_id,
            request,
        )
        .await
    }

    pub async fn update_user_credential_with_context<S>(
        &self,
        store: &S,
        context: AuthenticatedConfigurationContext,
        item_id: tanren_configuration_secrets::UserCredentialId,
        request: UpdateUserCredentialRequest,
    ) -> Result<UpdateUserCredentialResponse, AppServiceError>
    where
        S: UserConfigurationStore + AccountStore + ?Sized,
    {
        user_configuration::update_user_credential(store, &self.clock, context, item_id, request)
            .await
    }

    pub async fn list_user_credentials<S>(
        &self,
        store: &S,
        authenticated_account_id: AccountId,
        owner_scope: tanren_configuration_secrets::OwnerScope,
    ) -> Result<ListUserCredentialsResponse, AppServiceError>
    where
        S: UserConfigurationStore + ?Sized,
    {
        self.list_user_credentials_with_context(
            store,
            AuthenticatedConfigurationContext::for_requested_owner_scope(
                authenticated_account_id,
                owner_scope,
            ),
            ListUserCredentialsRequest::default(),
        )
        .await
    }

    pub async fn list_user_credentials_with_context<S>(
        &self,
        store: &S,
        context: AuthenticatedConfigurationContext,
        request: ListUserCredentialsRequest,
    ) -> Result<ListUserCredentialsResponse, AppServiceError>
    where
        S: UserConfigurationStore + ?Sized,
    {
        user_configuration::list_user_credentials(store, context, request).await
    }

    pub async fn list_user_credentials_page<S>(
        &self,
        store: &S,
        authenticated_account_id: AccountId,
        owner_scope: tanren_configuration_secrets::OwnerScope,
        request: ListUserCredentialsRequest,
    ) -> Result<ListUserCredentialsResponse, AppServiceError>
    where
        S: UserConfigurationStore + ?Sized,
    {
        self.list_user_credentials_with_context(
            store,
            AuthenticatedConfigurationContext::for_requested_owner_scope(
                authenticated_account_id,
                owner_scope,
            ),
            request,
        )
        .await
    }

    pub async fn remove_user_credential<S>(
        &self,
        store: &S,
        authenticated_account_id: AccountId,
        item_id: tanren_configuration_secrets::UserCredentialId,
        owner_scope: tanren_configuration_secrets::OwnerScope,
    ) -> Result<RemoveUserCredentialResponse, AppServiceError>
    where
        S: UserConfigurationStore + AccountStore + ?Sized,
    {
        self.remove_user_credential_with_context(
            store,
            AuthenticatedConfigurationContext::for_requested_owner_scope(
                authenticated_account_id,
                owner_scope,
            ),
            item_id,
        )
        .await
    }

    pub async fn remove_user_credential_with_context<S>(
        &self,
        store: &S,
        context: AuthenticatedConfigurationContext,
        item_id: tanren_configuration_secrets::UserCredentialId,
    ) -> Result<RemoveUserCredentialResponse, AppServiceError>
    where
        S: UserConfigurationStore + AccountStore + ?Sized,
    {
        user_configuration::remove_user_credential(store, &self.clock, context, item_id).await
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
