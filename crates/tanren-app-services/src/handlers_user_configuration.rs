use tanren_contract::{
    CreateUserCredentialRequest, CreateUserCredentialResponse, ListUserCredentialsRequest,
    ListUserCredentialsResponse, ListUserSettingsRequest, ListUserSettingsResponse, OwnerScope,
    RemoveUserCredentialResponse, RemoveUserSettingResponse, UpdateUserCredentialRequest,
    UpdateUserCredentialResponse, UpsertUserSettingRequest, UpsertUserSettingResponse,
    UserCredentialId, UserSettingKey,
};
use tanren_identity_policy::AccountId;
use tanren_store::{AccountStore, UserConfigurationStore};

use crate::credential_sealer::map_credential_sealer_error;
use crate::{AppServiceError, AuthenticatedConfigurationContext, Handlers, user_configuration};

impl Handlers {
    pub async fn list_user_settings<S>(
        &self,
        store: &S,
        authenticated_account_id: AccountId,
        requested_account_id: AccountId,
    ) -> Result<ListUserSettingsResponse, AppServiceError>
    where
        S: UserConfigurationStore + AccountStore + ?Sized,
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
        S: UserConfigurationStore + AccountStore + ?Sized,
    {
        user_configuration::list_user_settings(store, &self.clock, context, request).await
    }

    pub async fn list_user_settings_page<S>(
        &self,
        store: &S,
        authenticated_account_id: AccountId,
        requested_account_id: AccountId,
        request: ListUserSettingsRequest,
    ) -> Result<ListUserSettingsResponse, AppServiceError>
    where
        S: UserConfigurationStore + AccountStore + ?Sized,
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
        key: UserSettingKey,
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
        key: UserSettingKey,
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
        let sealer = self
            .value_sealing_state
            .as_result()
            .map_err(|err| map_credential_sealer_error(&err))?;
        user_configuration::add_user_credential(store, &self.clock, sealer, context, request).await
    }

    pub async fn update_user_credential<S>(
        &self,
        store: &S,
        authenticated_account_id: AccountId,
        item_id: UserCredentialId,
        owner_scope: OwnerScope,
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
        item_id: UserCredentialId,
        request: UpdateUserCredentialRequest,
    ) -> Result<UpdateUserCredentialResponse, AppServiceError>
    where
        S: UserConfigurationStore + AccountStore + ?Sized,
    {
        let sealer = self
            .value_sealing_state
            .as_result()
            .map_err(|err| map_credential_sealer_error(&err))?;
        user_configuration::update_user_credential(
            store,
            &self.clock,
            sealer,
            context,
            item_id,
            request,
        )
        .await
    }

    pub async fn list_user_credentials<S>(
        &self,
        store: &S,
        authenticated_account_id: AccountId,
        owner_scope: OwnerScope,
    ) -> Result<ListUserCredentialsResponse, AppServiceError>
    where
        S: UserConfigurationStore + AccountStore + ?Sized,
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
        S: UserConfigurationStore + AccountStore + ?Sized,
    {
        user_configuration::list_user_credentials(store, &self.clock, context, request).await
    }

    pub async fn list_user_credentials_page<S>(
        &self,
        store: &S,
        authenticated_account_id: AccountId,
        owner_scope: OwnerScope,
        request: ListUserCredentialsRequest,
    ) -> Result<ListUserCredentialsResponse, AppServiceError>
    where
        S: UserConfigurationStore + AccountStore + ?Sized,
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
        item_id: UserCredentialId,
        owner_scope: OwnerScope,
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
        item_id: UserCredentialId,
    ) -> Result<RemoveUserCredentialResponse, AppServiceError>
    where
        S: UserConfigurationStore + AccountStore + ?Sized,
    {
        user_configuration::remove_user_credential(store, &self.clock, context, item_id).await
    }
}
