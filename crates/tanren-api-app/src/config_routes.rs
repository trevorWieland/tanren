use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tanren_configuration_secrets::{CredentialId, UserSettingKey};
use tanren_contract::{
    ConfigurationFailureReason, CreateCredentialRequest, CreateCredentialResponse,
    GetUserConfigRequest, GetUserConfigResponse, ListCredentialsResponse, ListUserConfigResponse,
    RemoveCredentialRequest, RemoveUserConfigRequest, SetUserConfigRequest, SetUserConfigResponse,
    UpdateCredentialRequest, UpdateCredentialResponse,
};
use tanren_identity_policy::secret_serde;
use tower_sessions::Session;

use crate::AppState;
use crate::cookies::require_authenticated;
use crate::errors::{AccountFailureBody, ValidatedJson, map_app_error};

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub(crate) struct RemoveResult {
    pub removed: bool,
}

#[utoipa::path(
    get,
    path = "/me/config",
    responses(
        (status = 200, body = ListUserConfigResponse, description = "User-tier configuration entries"),
        (status = 401, body = AccountFailureBody, description = "unauthenticated"),
    ),
    tag = "user-config",
)]
pub(crate) async fn list_user_config_route(
    State(state): State<AppState>,
    session: Session,
) -> Response {
    let actor = match require_authenticated(&session).await {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    match state
        .handlers
        .list_user_config(state.store.as_ref(), &actor)
        .await
    {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(err) => map_app_error(err),
    }
}

#[utoipa::path(
    get,
    path = "/me/config/{key}",
    params(
        ("key" = String, Path, description = "User-tier setting key"),
    ),
    responses(
        (status = 200, body = GetUserConfigResponse, description = "User-tier configuration entry"),
        (status = 400, body = AccountFailureBody, description = "validation_failed"),
        (status = 401, body = AccountFailureBody, description = "unauthenticated"),
    ),
    tag = "user-config",
)]
pub(crate) async fn get_user_config_route(
    State(state): State<AppState>,
    session: Session,
    Path(key): Path<String>,
) -> Response {
    let actor = match require_authenticated(&session).await {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    let key = match parse_setting_key(&key) {
        Ok(k) => k,
        Err(resp) => return *resp,
    };
    match state
        .handlers
        .get_user_config(state.store.as_ref(), &actor, GetUserConfigRequest { key })
        .await
    {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(err) => map_app_error(err),
    }
}

#[utoipa::path(
    post,
    path = "/me/config",
    request_body = SetUserConfigRequest,
    responses(
        (status = 200, body = SetUserConfigResponse, description = "Setting upserted"),
        (status = 400, body = AccountFailureBody, description = "validation_failed"),
        (status = 401, body = AccountFailureBody, description = "unauthenticated"),
    ),
    tag = "user-config",
)]
pub(crate) async fn set_user_config_route(
    State(state): State<AppState>,
    session: Session,
    ValidatedJson(request): ValidatedJson<SetUserConfigRequest>,
) -> Response {
    let actor = match require_authenticated(&session).await {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    match state
        .handlers
        .set_user_config(state.store.as_ref(), &actor, request)
        .await
    {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(err) => map_app_error(err),
    }
}

#[utoipa::path(
    delete,
    path = "/me/config/{key}",
    params(
        ("key" = String, Path, description = "User-tier setting key"),
    ),
    responses(
        (status = 200, body = RemoveResult, description = "Setting removed"),
        (status = 400, body = AccountFailureBody, description = "validation_failed"),
        (status = 401, body = AccountFailureBody, description = "unauthenticated"),
    ),
    tag = "user-config",
)]
pub(crate) async fn remove_user_config_route(
    State(state): State<AppState>,
    session: Session,
    Path(key): Path<String>,
) -> Response {
    let actor = match require_authenticated(&session).await {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    let key = match parse_setting_key(&key) {
        Ok(k) => k,
        Err(resp) => return *resp,
    };
    match state
        .handlers
        .remove_user_config(
            state.store.as_ref(),
            &actor,
            RemoveUserConfigRequest { key },
        )
        .await
    {
        Ok(resp) => (
            StatusCode::OK,
            Json(RemoveResult {
                removed: resp.removed,
            }),
        )
            .into_response(),
        Err(err) => map_app_error(err),
    }
}

#[utoipa::path(
    get,
    path = "/me/credentials",
    responses(
        (status = 200, body = ListCredentialsResponse, description = "Credential metadata list"),
        (status = 401, body = AccountFailureBody, description = "unauthenticated"),
    ),
    tag = "credentials",
)]
pub(crate) async fn list_credentials_route(
    State(state): State<AppState>,
    session: Session,
) -> Response {
    let actor = match require_authenticated(&session).await {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    match state
        .handlers
        .list_credentials(state.store.as_ref(), &actor)
        .await
    {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(err) => map_app_error(err),
    }
}

#[utoipa::path(
    post,
    path = "/me/credentials",
    request_body = CreateCredentialRequest,
    responses(
        (status = 201, body = CreateCredentialResponse, description = "Credential created"),
        (status = 400, body = AccountFailureBody, description = "validation_failed"),
        (status = 401, body = AccountFailureBody, description = "unauthenticated"),
        (status = 409, body = AccountFailureBody, description = "duplicate_credential_name"),
    ),
    tag = "credentials",
)]
pub(crate) async fn create_credential_route(
    State(state): State<AppState>,
    session: Session,
    ValidatedJson(request): ValidatedJson<CreateCredentialRequest>,
) -> Response {
    let actor = match require_authenticated(&session).await {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    match state
        .handlers
        .create_credential(state.store.as_ref(), &actor, request)
        .await
    {
        Ok(resp) => (StatusCode::CREATED, Json(resp)).into_response(),
        Err(err) => map_app_error(err),
    }
}

#[utoipa::path(
    patch,
    path = "/me/credentials/{id}",
    request_body = UpdateCredentialBody,
    params(
        ("id" = String, Path, description = "Credential UUID"),
    ),
    responses(
        (status = 200, body = UpdateCredentialResponse, description = "Credential updated"),
        (status = 400, body = AccountFailureBody, description = "validation_failed"),
        (status = 401, body = AccountFailureBody, description = "unauthenticated"),
        (status = 403, body = AccountFailureBody, description = "unauthorized"),
        (status = 404, body = AccountFailureBody, description = "credential_not_found"),
    ),
    tag = "credentials",
)]
pub(crate) async fn update_credential_route(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<CredentialId>,
    ValidatedJson(body): ValidatedJson<UpdateCredentialBody>,
) -> Response {
    let actor = match require_authenticated(&session).await {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    let request = UpdateCredentialRequest {
        id,
        name: body.name,
        description: body.description,
        value: body.value,
    };
    match state
        .handlers
        .update_credential(state.store.as_ref(), &actor, request)
        .await
    {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(err) => map_app_error(err),
    }
}

#[utoipa::path(
    delete,
    path = "/me/credentials/{id}",
    params(
        ("id" = String, Path, description = "Credential UUID"),
    ),
    responses(
        (status = 200, body = RemoveResult, description = "Credential removed"),
        (status = 401, body = AccountFailureBody, description = "unauthenticated"),
        (status = 403, body = AccountFailureBody, description = "unauthorized"),
        (status = 404, body = AccountFailureBody, description = "credential_not_found"),
    ),
    tag = "credentials",
)]
pub(crate) async fn remove_credential_route(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<CredentialId>,
) -> Response {
    let actor = match require_authenticated(&session).await {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    match state
        .handlers
        .remove_credential(state.store.as_ref(), &actor, RemoveCredentialRequest { id })
        .await
    {
        Ok(resp) => (
            StatusCode::OK,
            Json(RemoveResult {
                removed: resp.removed,
            }),
        )
            .into_response(),
        Err(err) => map_app_error(err),
    }
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub(crate) struct UpdateCredentialBody {
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(deserialize_with = "secret_serde::deserialize_password")]
    #[schema(value_type = String)]
    pub value: SecretString,
}

fn parse_setting_key(raw: &str) -> Result<UserSettingKey, Box<Response>> {
    let json_value = serde_json::Value::String(raw.to_owned());
    serde_json::from_value::<UserSettingKey>(json_value).map_err(|_| {
        Box::new(
            (
                StatusCode::BAD_REQUEST,
                Json(AccountFailureBody {
                    code: ConfigurationFailureReason::InvalidSettingKey
                        .code()
                        .to_owned(),
                    summary: ConfigurationFailureReason::InvalidSettingKey
                        .summary()
                        .to_owned(),
                }),
            )
                .into_response(),
        )
    })
}
