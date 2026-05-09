use axum::Json;
use axum::extract::rejection::QueryRejection;
use axum::extract::{FromRequestParts, Path, Query, State};
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use secrecy::SecretString;
use serde::Deserialize;
use tanren_configuration_secrets::{UserCredentialKind, UserSettingKey, UserSettingValue};
use tanren_contract::{
    ConfigurationCapabilitiesView, CreateUserCredentialRequest, CreateUserCredentialResponse,
    CredentialCapabilitiesView, CredentialCapabilityAction,
    GetAuthenticatedUserConfigurationCapabilitiesResponse, ListUserCredentialsResponse,
    ListUserSettingsResponse, RemoveUserCredentialResponse, RemoveUserSettingResponse,
    SettingCapabilitiesView, SettingCapabilityAction, UpdateUserCredentialRequest,
    UpdateUserCredentialResponse, UpsertUserSettingRequest, UpsertUserSettingResponse,
};
use tanren_identity_policy::{AccountId, secret_serde};
use tower_sessions::Session;
use utoipa::ToSchema;

use super::user_configuration::{
    ConfigurationListQuery, authenticated_account_id, internal_error,
    list_credentials_request_from_query, list_settings_request_from_query, owner_scope_for,
    parse_user_credential_id, parse_user_setting_key, validation_failed,
};
use super::user_configuration_helpers::{
    add_user_credential_operation, list_user_credentials_operation, list_user_settings_operation,
    remove_user_credential_operation, remove_user_setting_operation,
    update_user_credential_operation, upsert_user_setting_operation,
};
use crate::AppState;
use crate::errors::{AccountFailureBody, ValidatedJson};

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AuthenticatedUpsertUserSettingRequest {
    pub key: UserSettingKey,
    pub value: UserSettingValue,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AuthenticatedCreateUserCredentialRequest {
    pub kind: UserCredentialKind,
    #[serde(deserialize_with = "secret_serde::deserialize_password")]
    #[schema(value_type = String, format = Password)]
    pub value: SecretString,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AuthenticatedUpdateUserCredentialRequest {
    #[serde(deserialize_with = "secret_serde::deserialize_password")]
    #[schema(value_type = String, format = Password)]
    pub value: SecretString,
}

#[utoipa::path(
    get,
    path = "/configuration/account/user-settings",
    params(
        ("limit" = Option<u16>, Query, description = "Bounded page size"),
        ("after" = Option<String>, Query, description = "Opaque cursor from previous page"),
    ),
    responses(
        (status = 200, body = ListUserSettingsResponse),
        (status = 401, body = AccountFailureBody, description = "auth_required"),
        (status = 404, body = AccountFailureBody, description = "setting_not_found"),
    ),
    tag = "configuration",
)]
pub(crate) async fn list_authenticated_user_settings_route(
    State(state): State<AppState>,
    account_scope: AuthenticatedAccountScope,
    query: Result<Query<ConfigurationListQuery>, QueryRejection>,
) -> Response {
    let Ok(request) = list_settings_request_from_query(query) else {
        return validation_failed("query parameters are invalid");
    };
    list_user_settings_operation(&state, account_scope.settings_context(), request).await
}

#[utoipa::path(
    post,
    path = "/configuration/account/user-settings",
    request_body = AuthenticatedUpsertUserSettingRequest,
    responses(
        (status = 200, body = UpsertUserSettingResponse),
        (status = 400, body = AccountFailureBody, description = "validation_failed"),
        (status = 401, body = AccountFailureBody, description = "auth_required"),
        (status = 404, body = AccountFailureBody, description = "setting_not_found"),
    ),
    tag = "configuration",
)]
pub(crate) async fn upsert_authenticated_user_setting_route(
    State(state): State<AppState>,
    account_scope: AuthenticatedAccountScope,
    ValidatedJson(request): ValidatedJson<AuthenticatedUpsertUserSettingRequest>,
) -> Response {
    let request = UpsertUserSettingRequest {
        key: request.key,
        value: request.value,
    };
    upsert_user_setting_operation(&state, account_scope.settings_context(), request).await
}

#[utoipa::path(
    delete,
    path = "/configuration/account/user-settings/{key}",
    params(
        ("key" = UserSettingKey, Path, description = "User setting key to remove"),
    ),
    responses(
        (status = 200, body = RemoveUserSettingResponse),
        (status = 400, body = AccountFailureBody, description = "validation_failed"),
        (status = 401, body = AccountFailureBody, description = "auth_required"),
        (status = 404, body = AccountFailureBody, description = "setting_not_found"),
    ),
    tag = "configuration",
)]
pub(crate) async fn remove_authenticated_user_setting_route(
    State(state): State<AppState>,
    account_scope: AuthenticatedAccountScope,
    Path(key_raw): Path<String>,
) -> Response {
    let key = match parse_user_setting_key(&key_raw) {
        Ok(value) => value,
        Err(message) => return validation_failed(message),
    };
    remove_user_setting_operation(&state, account_scope.settings_context(), key).await
}

#[utoipa::path(
    post,
    path = "/configuration/account/user-credentials",
    request_body = AuthenticatedCreateUserCredentialRequest,
    responses(
        (status = 201, body = CreateUserCredentialResponse),
        (status = 400, body = AccountFailureBody, description = "validation_failed"),
        (status = 401, body = AccountFailureBody, description = "auth_required"),
        (status = 404, body = AccountFailureBody, description = "item_not_found"),
    ),
    tag = "configuration",
)]
pub(crate) async fn add_authenticated_user_credential_route(
    State(state): State<AppState>,
    account_scope: AuthenticatedAccountScope,
    ValidatedJson(request): ValidatedJson<AuthenticatedCreateUserCredentialRequest>,
) -> Response {
    let request = CreateUserCredentialRequest {
        kind: request.kind,
        owner_scope: owner_scope_for(account_scope.account_id()),
        value: request.value,
    };
    add_user_credential_operation(&state, account_scope.credential_context(), request).await
}

#[utoipa::path(
    put,
    path = "/configuration/account/user-credentials/{item_id}",
    params(
        ("item_id" = String, Path, description = "Credential metadata id"),
    ),
    request_body = AuthenticatedUpdateUserCredentialRequest,
    responses(
        (status = 200, body = UpdateUserCredentialResponse),
        (status = 400, body = AccountFailureBody, description = "validation_failed"),
        (status = 401, body = AccountFailureBody, description = "auth_required"),
        (status = 404, body = AccountFailureBody, description = "item_not_found"),
    ),
    tag = "configuration",
)]
pub(crate) async fn update_authenticated_user_credential_route(
    State(state): State<AppState>,
    account_scope: AuthenticatedAccountScope,
    Path(item_id): Path<String>,
    ValidatedJson(request): ValidatedJson<AuthenticatedUpdateUserCredentialRequest>,
) -> Response {
    let item_id = match parse_user_credential_id(&item_id) {
        Ok(value) => value,
        Err(message) => return validation_failed(message),
    };
    update_user_credential_operation(
        &state,
        account_scope.credential_context(),
        item_id,
        UpdateUserCredentialRequest {
            value: request.value,
        },
    )
    .await
}

#[utoipa::path(
    get,
    path = "/configuration/account/user-credentials",
    params(
        ("limit" = Option<u16>, Query, description = "Bounded page size"),
        ("after" = Option<String>, Query, description = "Opaque cursor from previous page"),
    ),
    responses(
        (status = 200, body = ListUserCredentialsResponse),
        (status = 401, body = AccountFailureBody, description = "auth_required"),
        (status = 404, body = AccountFailureBody, description = "item_not_found"),
    ),
    tag = "configuration",
)]
pub(crate) async fn list_authenticated_user_credentials_route(
    State(state): State<AppState>,
    account_scope: AuthenticatedAccountScope,
    query: Result<Query<ConfigurationListQuery>, QueryRejection>,
) -> Response {
    let Ok(request) = list_credentials_request_from_query(query) else {
        return validation_failed("query parameters are invalid");
    };
    list_user_credentials_operation(&state, account_scope.credential_context(), request).await
}

#[utoipa::path(
    delete,
    path = "/configuration/account/user-credentials/{item_id}",
    params(
        ("item_id" = String, Path, description = "Credential metadata id"),
    ),
    responses(
        (status = 200, body = RemoveUserCredentialResponse),
        (status = 401, body = AccountFailureBody, description = "auth_required"),
        (status = 404, body = AccountFailureBody, description = "item_not_found"),
    ),
    tag = "configuration",
)]
pub(crate) async fn remove_authenticated_user_credential_route(
    State(state): State<AppState>,
    account_scope: AuthenticatedAccountScope,
    Path(item_id): Path<String>,
) -> Response {
    let item_id = match parse_user_credential_id(&item_id) {
        Ok(value) => value,
        Err(message) => return validation_failed(message),
    };
    remove_user_credential_operation(&state, account_scope.credential_context(), item_id).await
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct AuthenticatedAccountScope(AccountId);

impl AuthenticatedAccountScope {
    const fn account_id(self) -> AccountId {
        self.0
    }

    const fn settings_context(self) -> tanren_app_services::AuthenticatedConfigurationContext {
        tanren_app_services::AuthenticatedConfigurationContext::for_requested_account(
            self.0, self.0,
        )
    }

    const fn credential_context(self) -> tanren_app_services::AuthenticatedConfigurationContext {
        tanren_app_services::AuthenticatedConfigurationContext::for_requested_owner_scope(
            self.0,
            owner_scope_for(self.0),
        )
    }
}

impl<S> FromRequestParts<S> for AuthenticatedAccountScope
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(parts, state)
            .await
            .map_err(|_| internal_error())?;
        let account_id = authenticated_account_id(&session).await?;
        Ok(Self(account_id))
    }
}

#[utoipa::path(
    get,
    path = "/configuration/account/capabilities",
    responses(
        (status = 200, body = GetAuthenticatedUserConfigurationCapabilitiesResponse),
        (status = 401, body = AccountFailureBody, description = "auth_required"),
    ),
    tag = "configuration",
)]
pub(crate) async fn get_authenticated_user_configuration_capabilities_route(
    AuthenticatedAccountScope(_account_id): AuthenticatedAccountScope,
) -> Response {
    (
        StatusCode::OK,
        Json(GetAuthenticatedUserConfigurationCapabilitiesResponse {
            capabilities: ConfigurationCapabilitiesView {
                settings: SettingCapabilitiesView {
                    allowed_actions: vec![
                        SettingCapabilityAction::Read,
                        SettingCapabilityAction::CreateOrUpdate,
                        SettingCapabilityAction::Delete,
                    ],
                },
                user_items: CredentialCapabilitiesView {
                    allowed_actions: vec![
                        CredentialCapabilityAction::Read,
                        CredentialCapabilityAction::Create,
                        CredentialCapabilityAction::Update,
                        CredentialCapabilityAction::Delete,
                    ],
                },
            },
        }),
    )
        .into_response()
}
