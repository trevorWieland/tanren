use axum::Json;
use axum::extract::{FromRequestParts, Path, State};
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use secrecy::SecretString;
use serde::Deserialize;
use tanren_configuration_secrets::{
    OwnerScope, UserCredentialKind, UserSettingKey, UserSettingValue,
};
use tanren_contract::{
    CreateUserCredentialRequest, CreateUserCredentialResponse, ListUserCredentialsResponse,
    ListUserSettingsResponse, RemoveUserCredentialResponse, RemoveUserSettingResponse,
    UpdateUserCredentialRequest, UpdateUserCredentialResponse, UpsertUserSettingRequest,
    UpsertUserSettingResponse,
};
use tanren_identity_policy::{AccountId, secret_serde};
use tower_sessions::Session;
use utoipa::ToSchema;

use crate::AppState;
use crate::cookies::SESSION_KEY_ACCOUNT;
use crate::errors::{AccountFailureBody, ValidatedJson, map_app_error};

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
    responses(
        (status = 200, body = ListUserSettingsResponse),
        (status = 401, body = AccountFailureBody, description = "auth_required"),
        (status = 404, body = AccountFailureBody, description = "setting_not_found"),
    ),
    tag = "configuration",
)]
pub(crate) async fn list_authenticated_user_settings_route(
    State(state): State<AppState>,
    AuthenticatedAccountScope(account_id): AuthenticatedAccountScope,
) -> Response {
    match state
        .handlers
        .list_user_settings(state.store.as_ref(), account_id, account_id)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
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
    AuthenticatedAccountScope(account_id): AuthenticatedAccountScope,
    ValidatedJson(request): ValidatedJson<AuthenticatedUpsertUserSettingRequest>,
) -> Response {
    let request = UpsertUserSettingRequest {
        key: request.key,
        value: request.value,
    };
    match state
        .handlers
        .upsert_user_setting(state.store.as_ref(), account_id, account_id, request)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
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
    AuthenticatedAccountScope(account_id): AuthenticatedAccountScope,
    Path(key_raw): Path<String>,
) -> Response {
    let key = match parse_user_setting_key(&key_raw) {
        Ok(value) => value,
        Err(message) => return validation_failed(message),
    };
    match state
        .handlers
        .remove_user_setting(state.store.as_ref(), account_id, account_id, key)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
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
    AuthenticatedAccountScope(account_id): AuthenticatedAccountScope,
    ValidatedJson(request): ValidatedJson<AuthenticatedCreateUserCredentialRequest>,
) -> Response {
    let request = CreateUserCredentialRequest {
        kind: request.kind,
        owner_scope: owner_scope_for(account_id),
        value: request.value,
    };
    match state
        .handlers
        .add_user_credential(state.store.as_ref(), account_id, request)
        .await
    {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
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
    AuthenticatedAccountScope(account_id): AuthenticatedAccountScope,
    Path(item_id): Path<String>,
    ValidatedJson(request): ValidatedJson<AuthenticatedUpdateUserCredentialRequest>,
) -> Response {
    match state
        .handlers
        .update_user_credential(
            state.store.as_ref(),
            account_id,
            &item_id,
            owner_scope_for(account_id),
            UpdateUserCredentialRequest {
                value: request.value,
            },
        )
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
}

#[utoipa::path(
    get,
    path = "/configuration/account/user-credentials",
    responses(
        (status = 200, body = ListUserCredentialsResponse),
        (status = 401, body = AccountFailureBody, description = "auth_required"),
        (status = 404, body = AccountFailureBody, description = "item_not_found"),
    ),
    tag = "configuration",
)]
pub(crate) async fn list_authenticated_user_credentials_route(
    State(state): State<AppState>,
    AuthenticatedAccountScope(account_id): AuthenticatedAccountScope,
) -> Response {
    match state
        .handlers
        .list_user_credentials(
            state.store.as_ref(),
            account_id,
            owner_scope_for(account_id),
        )
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
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
    AuthenticatedAccountScope(account_id): AuthenticatedAccountScope,
    Path(item_id): Path<String>,
) -> Response {
    match state
        .handlers
        .remove_user_credential(
            state.store.as_ref(),
            account_id,
            &item_id,
            owner_scope_for(account_id),
        )
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct AuthenticatedAccountScope(AccountId);

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

async fn authenticated_account_id(session: &Session) -> Result<AccountId, Response> {
    match session.get::<AccountId>(SESSION_KEY_ACCOUNT).await {
        Ok(Some(account_id)) => Ok(account_id),
        Ok(None) => Err(auth_required()),
        Err(err) => {
            tracing::error!(target: "tanren_api", error = %err, "session read account_id");
            Err(internal_error())
        }
    }
}

fn auth_required() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(AccountFailureBody {
            code: "auth_required".to_owned(),
            summary: "An authenticated session is required.".to_owned(),
        }),
    )
        .into_response()
}

fn internal_error() -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(AccountFailureBody {
            code: "internal_error".to_owned(),
            summary: "Tanren encountered an internal error.".to_owned(),
        }),
    )
        .into_response()
}

fn validation_failed(summary: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(AccountFailureBody {
            code: "validation_failed".to_owned(),
            summary: summary.to_owned(),
        }),
    )
        .into_response()
}

fn parse_user_setting_key(raw: &str) -> Result<UserSettingKey, &'static str> {
    match raw {
        "theme" => Ok(UserSettingKey::Theme),
        "editor" => Ok(UserSettingKey::Editor),
        _ => Err("key must be one of: theme, editor"),
    }
}

fn owner_scope_for(account_id: AccountId) -> OwnerScope {
    OwnerScope::User { account_id }
}
