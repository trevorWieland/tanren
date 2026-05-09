use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use tanren_app_services::AuthenticatedConfigurationContext;
use tanren_configuration_secrets::{UserCredentialId, UserSettingKey};
use tanren_contract::{
    CreateUserCredentialRequest, ListUserCredentialsRequest, ListUserSettingsRequest,
    UpdateUserCredentialRequest, UpsertUserSettingRequest,
};

use crate::AppState;
use crate::errors::map_app_error;

pub(super) async fn list_user_settings_operation(
    state: &AppState,
    context: AuthenticatedConfigurationContext,
    request: ListUserSettingsRequest,
) -> Response {
    match state
        .handlers
        .list_user_settings_with_context(state.store.as_ref(), context, request)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
}

pub(super) async fn upsert_user_setting_operation(
    state: &AppState,
    context: AuthenticatedConfigurationContext,
    request: UpsertUserSettingRequest,
) -> Response {
    match state
        .handlers
        .upsert_user_setting_with_context(state.store.as_ref(), context, request)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
}

pub(super) async fn remove_user_setting_operation(
    state: &AppState,
    context: AuthenticatedConfigurationContext,
    key: UserSettingKey,
) -> Response {
    match state
        .handlers
        .remove_user_setting_with_context(state.store.as_ref(), context, key)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
}

pub(super) async fn add_user_credential_operation(
    state: &AppState,
    context: AuthenticatedConfigurationContext,
    request: CreateUserCredentialRequest,
) -> Response {
    match state
        .handlers
        .add_user_credential_with_context(state.store.as_ref(), context, request)
        .await
    {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
}

pub(super) async fn update_user_credential_operation(
    state: &AppState,
    context: AuthenticatedConfigurationContext,
    item_id: UserCredentialId,
    request: UpdateUserCredentialRequest,
) -> Response {
    match state
        .handlers
        .update_user_credential_with_context(state.store.as_ref(), context, item_id, request)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
}

pub(super) async fn list_user_credentials_operation(
    state: &AppState,
    context: AuthenticatedConfigurationContext,
    request: ListUserCredentialsRequest,
) -> Response {
    match state
        .handlers
        .list_user_credentials_with_context(state.store.as_ref(), context, request)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
}

pub(super) async fn remove_user_credential_operation(
    state: &AppState,
    context: AuthenticatedConfigurationContext,
    item_id: UserCredentialId,
) -> Response {
    match state
        .handlers
        .remove_user_credential_with_context(state.store.as_ref(), context, item_id)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
}
