use axum::Json;
use axum::extract::rejection::QueryRejection;
use axum::extract::{FromRequestParts, Path, Query, State};
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashMap;
use tanren_app_services::AuthenticatedConfigurationContext;
use tanren_contract::{
    CreateUserCredentialRequest, CreateUserCredentialResponse, ListUserCredentialsRequest,
    ListUserCredentialsResponse, ListUserSettingsRequest, ListUserSettingsResponse, OwnerScope,
    RemoveUserCredentialResponse, RemoveUserSettingResponse, UpdateUserCredentialRequest,
    UpdateUserCredentialResponse, UpsertUserSettingRequest, UpsertUserSettingResponse,
    UserConfigurationFailureReason, UserCredentialId, UserSettingKey,
    parse_user_setting_key as parse_user_setting_key_registry,
};
use tanren_identity_policy::AccountId;
use tower_sessions::Session;
use uuid::Uuid;

use super::user_configuration_helpers::{
    add_user_credential_operation, list_user_credentials_operation, list_user_settings_operation,
    remove_user_credential_operation, remove_user_setting_operation,
    update_user_credential_operation, upsert_user_setting_operation,
};
use crate::AppState;
use crate::cookies::{SESSION_KEY_ACCOUNT, SESSION_KEY_EXPIRES};
use crate::errors::AccountFailureBody;
use crate::errors::ValidatedJson;

#[utoipa::path(
    get,
    path = "/accounts/{account_id}/user-settings",
    params(
        ("account_id" = AccountId, Path, description = "Target account id"),
        ("limit" = Option<u16>, Query, description = "Bounded page size"),
        ("after" = Option<String>, Query, description = "Opaque cursor from previous page"),
    ),
    responses(
        (status = 200, body = ListUserSettingsResponse),
        (status = 400, body = AccountFailureBody, description = "validation_failed"),
        (status = 401, body = AccountFailureBody, description = "auth_required"),
        (status = 404, body = AccountFailureBody, description = "setting_not_found"),
    ),
    tag = "configuration",
)]
pub(crate) async fn list_user_settings_route(
    State(state): State<AppState>,
    SettingAccountScope(scope): SettingAccountScope,
    query: Result<Query<ConfigurationListQuery>, QueryRejection>,
) -> Response {
    let Ok(request) = list_settings_request_from_query(query) else {
        return validation_failed("query parameters are invalid");
    };
    list_user_settings_operation(&state, scope.context(), request).await
}

#[utoipa::path(
    post,
    path = "/accounts/{account_id}/user-settings",
    params(
        ("account_id" = AccountId, Path, description = "Target account id"),
        ("limit" = Option<u16>, Query, description = "Bounded page size"),
        ("after" = Option<String>, Query, description = "Opaque cursor from previous page"),
    ),
    request_body = UpsertUserSettingRequest,
    responses(
        (status = 200, body = UpsertUserSettingResponse),
        (status = 400, body = AccountFailureBody, description = "validation_failed"),
        (status = 401, body = AccountFailureBody, description = "auth_required"),
        (status = 404, body = AccountFailureBody, description = "setting_not_found"),
    ),
    tag = "configuration",
)]
pub(crate) async fn upsert_user_setting_route(
    State(state): State<AppState>,
    SettingAccountScope(scope): SettingAccountScope,
    ValidatedJson(request): ValidatedJson<UpsertUserSettingRequest>,
) -> Response {
    upsert_user_setting_operation(&state, scope.context(), request).await
}

#[utoipa::path(
    delete,
    path = "/accounts/{account_id}/user-settings/{key}",
    params(
        ("account_id" = AccountId, Path, description = "Target account id"),
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
pub(crate) async fn remove_user_setting_route(
    State(state): State<AppState>,
    SettingAccountScope(scope): SettingAccountScope,
    Path(path_params): Path<(String, String)>,
) -> Response {
    let (_, key_raw) = path_params;
    let key = match parse_user_setting_key(&key_raw) {
        Ok(value) => value,
        Err(message) => return validation_failed(message),
    };
    remove_user_setting_operation(&state, scope.context(), key).await
}

#[utoipa::path(
    post,
    path = "/accounts/{account_id}/user-credentials",
    params(
        ("account_id" = AccountId, Path, description = "Target account id"),
    ),
    request_body = CreateUserCredentialRequest,
    responses(
        (status = 201, body = CreateUserCredentialResponse),
        (status = 400, body = AccountFailureBody, description = "validation_failed"),
        (status = 401, body = AccountFailureBody, description = "auth_required"),
        (status = 404, body = AccountFailureBody, description = "item_not_found"),
    ),
    tag = "configuration",
)]
pub(crate) async fn add_user_credential_route(
    State(state): State<AppState>,
    CredentialAccountScope(scope): CredentialAccountScope,
    ValidatedJson(request): ValidatedJson<CreateUserCredentialRequest>,
) -> Response {
    let expected_scope = OwnerScope::User {
        account_id: scope.requested,
    };
    if request.owner_scope != expected_scope {
        return validation_failed("owner_scope must match path account_id");
    }
    add_user_credential_operation(&state, scope.context(), request).await
}

#[utoipa::path(
    put,
    path = "/accounts/{account_id}/user-credentials/{item_id}",
    params(
        ("account_id" = AccountId, Path, description = "Target account id"),
        ("item_id" = String, Path, description = "Credential metadata id"),
    ),
    request_body = UpdateUserCredentialRequest,
    responses(
        (status = 200, body = UpdateUserCredentialResponse),
        (status = 400, body = AccountFailureBody, description = "validation_failed"),
        (status = 401, body = AccountFailureBody, description = "auth_required"),
        (status = 404, body = AccountFailureBody, description = "item_not_found"),
    ),
    tag = "configuration",
)]
pub(crate) async fn update_user_credential_route(
    State(state): State<AppState>,
    CredentialAccountScope(scope): CredentialAccountScope,
    Path(path_params): Path<(String, String)>,
    ValidatedJson(request): ValidatedJson<UpdateUserCredentialRequest>,
) -> Response {
    let (_, item_id) = path_params;
    let item_id = match parse_user_credential_id(&item_id) {
        Ok(value) => value,
        Err(message) => return validation_failed(message),
    };
    update_user_credential_operation(&state, scope.context(), item_id, request).await
}

#[utoipa::path(
    get,
    path = "/accounts/{account_id}/user-credentials",
    params(
        ("account_id" = AccountId, Path, description = "Target account id"),
    ),
    responses(
        (status = 200, body = ListUserCredentialsResponse),
        (status = 400, body = AccountFailureBody, description = "validation_failed"),
        (status = 401, body = AccountFailureBody, description = "auth_required"),
        (status = 404, body = AccountFailureBody, description = "item_not_found"),
    ),
    tag = "configuration",
)]
pub(crate) async fn list_user_credentials_route(
    State(state): State<AppState>,
    CredentialAccountScope(scope): CredentialAccountScope,
    query: Result<Query<ConfigurationListQuery>, QueryRejection>,
) -> Response {
    let Ok(request) = list_credentials_request_from_query(query) else {
        return validation_failed("query parameters are invalid");
    };
    list_user_credentials_operation(&state, scope.context(), request).await
}

#[utoipa::path(
    delete,
    path = "/accounts/{account_id}/user-credentials/{item_id}",
    params(
        ("account_id" = AccountId, Path, description = "Target account id"),
        ("item_id" = String, Path, description = "Credential metadata id"),
    ),
    responses(
        (status = 200, body = RemoveUserCredentialResponse),
        (status = 400, body = AccountFailureBody, description = "validation_failed"),
        (status = 401, body = AccountFailureBody, description = "auth_required"),
        (status = 404, body = AccountFailureBody, description = "item_not_found"),
    ),
    tag = "configuration",
)]
pub(crate) async fn remove_user_credential_route(
    State(state): State<AppState>,
    CredentialAccountScope(scope): CredentialAccountScope,
    Path(path_params): Path<(String, String)>,
) -> Response {
    let (_, item_id) = path_params;
    let item_id = match parse_user_credential_id(&item_id) {
        Ok(value) => value,
        Err(message) => return validation_failed(message),
    };
    remove_user_credential_operation(&state, scope.context(), item_id).await
}

#[derive(Debug, Clone, Copy)]
struct AuthorizedAccount {
    authenticated: AccountId,
    requested: AccountId,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConfigurationListQuery {
    limit: Option<u16>,
    after: Option<String>,
}

impl AuthorizedAccount {
    pub(super) const fn context(self) -> AuthenticatedConfigurationContext {
        AuthenticatedConfigurationContext::for_requested_account(self.authenticated, self.requested)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SettingAccountScope(AuthorizedAccount);

impl<S> FromRequestParts<S> for SettingAccountScope
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        authorize_account_from_parts(
            parts,
            state,
            UserConfigurationFailureReason::SettingNotFound,
        )
        .await
        .map(Self)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CredentialAccountScope(AuthorizedAccount);

impl<S> FromRequestParts<S> for CredentialAccountScope
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        authorize_account_from_parts(parts, state, UserConfigurationFailureReason::ItemNotFound)
            .await
            .map(Self)
    }
}

async fn authorize_account_from_parts<S>(
    parts: &mut Parts,
    state: &S,
    denial: UserConfigurationFailureReason,
) -> Result<AuthorizedAccount, Response>
where
    S: Send + Sync,
{
    let Path(path_params): Path<HashMap<String, String>> = Path::from_request_parts(parts, state)
        .await
        .map_err(|_| validation_failed("account_id must be a valid uuid"))?;
    let Some(account_id_raw) = path_params.get("account_id") else {
        return Err(validation_failed("account_id must be a valid uuid"));
    };
    let requested = match parse_account_id(account_id_raw) {
        Ok(value) => value,
        Err(message) => return Err(validation_failed(message)),
    };

    let session = Session::from_request_parts(parts, state)
        .await
        .map_err(|_| internal_error())?;
    let authenticated = authenticated_account_id(&session).await?;

    if !is_same_account(requested, authenticated) {
        return Err(configuration_failure(&denial));
    }

    Ok(AuthorizedAccount {
        authenticated,
        requested,
    })
}

pub(super) fn list_settings_request_from_query(
    query: Result<Query<ConfigurationListQuery>, QueryRejection>,
) -> Result<ListUserSettingsRequest, ()> {
    let Ok(Query(value)) = query else {
        return Err(());
    };
    Ok(ListUserSettingsRequest {
        limit: value.limit,
        after: value.after,
    })
}

pub(super) fn list_credentials_request_from_query(
    query: Result<Query<ConfigurationListQuery>, QueryRejection>,
) -> Result<ListUserCredentialsRequest, ()> {
    let Ok(Query(value)) = query else {
        return Err(());
    };
    Ok(ListUserCredentialsRequest {
        limit: value.limit,
        after: value.after,
    })
}

pub(super) async fn authenticated_account_id(session: &Session) -> Result<AccountId, Response> {
    authenticated_session_context(session)
        .await
        .map(|context| context.account_id)
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct AuthenticatedSessionContext {
    pub(crate) account_id: AccountId,
    pub(crate) expires_at: DateTime<Utc>,
}

pub(crate) async fn authenticated_session_context(
    session: &Session,
) -> Result<AuthenticatedSessionContext, Response> {
    let account_id = match session.get::<AccountId>(SESSION_KEY_ACCOUNT).await {
        Ok(Some(value)) => value,
        Ok(None) => return Err(auth_required()),
        Err(err) => {
            tracing::error!(target: "tanren_api", error = %err, "session read account_id");
            return Err(internal_error());
        }
    };
    let expires_at = match session.get::<DateTime<Utc>>(SESSION_KEY_EXPIRES).await {
        Ok(Some(value)) => value,
        Ok(None) => return Err(auth_required()),
        Err(err) => {
            tracing::error!(target: "tanren_api", error = %err, "session read expires_at");
            return Err(internal_error());
        }
    };
    Ok(AuthenticatedSessionContext {
        account_id,
        expires_at,
    })
}

pub(super) fn auth_required() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(AccountFailureBody {
            code: "auth_required".to_owned(),
            summary: "An authenticated session is required.".to_owned(),
        }),
    )
        .into_response()
}

pub(super) fn internal_error() -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(AccountFailureBody {
            code: "internal_error".to_owned(),
            summary: "Tanren encountered an internal error.".to_owned(),
        }),
    )
        .into_response()
}

pub(super) fn validation_failed(summary: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(AccountFailureBody {
            code: "validation_failed".to_owned(),
            summary: summary.to_owned(),
        }),
    )
        .into_response()
}

fn parse_account_id(raw: &str) -> Result<AccountId, &'static str> {
    Uuid::parse_str(raw)
        .map(AccountId::new)
        .map_err(|_| "account_id must be a valid uuid")
}

pub(super) fn parse_user_setting_key(raw: &str) -> Result<UserSettingKey, &'static str> {
    parse_user_setting_key_registry(raw).map_err(|_| "key must be one of: theme, editor")
}

pub(super) fn parse_user_credential_id(raw: &str) -> Result<UserCredentialId, &'static str> {
    UserCredentialId::parse(raw).map_err(|_| "item_id must be a valid uuid")
}

pub(super) const fn owner_scope_for(account_id: AccountId) -> OwnerScope {
    OwnerScope::User { account_id }
}

fn is_same_account(requested_account_id: AccountId, authenticated_account_id: AccountId) -> bool {
    requested_account_id == authenticated_account_id
}

fn configuration_failure(reason: &UserConfigurationFailureReason) -> Response {
    let status =
        StatusCode::from_u16(reason.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (
        status,
        Json(AccountFailureBody {
            code: reason.code().to_owned(),
            summary: reason.summary().to_owned(),
        }),
    )
        .into_response()
}
