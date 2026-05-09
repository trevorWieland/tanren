use reqwest::{Client, Method};
use secrecy::ExposeSecret;
use serde_json::Value;
use tanren_configuration_secrets::OwnerScope;
use tanren_contract::{
    CreateUserCredentialRequest, CreateUserCredentialResponse, ListUserCredentialsResponse,
    ListUserSettingsResponse, RemoveUserCredentialResponse, UpsertUserSettingRequest,
    UpsertUserSettingResponse,
};
use tanren_identity_policy::AccountId;

use super::request_retry::{send, send_get_with_retry, send_json};
use super::{HarnessError, HarnessResult, failure_from_body};

pub(super) async fn list_user_settings(
    base_url: &str,
    client: &Client,
    requested_account_id: AccountId,
) -> HarnessResult<ListUserSettingsResponse> {
    let url = format!("{base_url}/accounts/{requested_account_id}/user-settings");
    let response = send_get_with_retry(client, &url, "GET /accounts/{id}/user-settings").await?;
    let status = response.status();
    let json: Value = response
        .json()
        .await
        .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
    if !status.is_success() {
        return Err(failure_from_body(&json));
    }
    serde_json::from_value(json)
        .map_err(|e| HarnessError::Transport(format!("decode settings response: {e}")))
}

pub(super) async fn upsert_user_setting(
    base_url: &str,
    client: &Client,
    requested_account_id: AccountId,
    request: UpsertUserSettingRequest,
) -> HarnessResult<UpsertUserSettingResponse> {
    let url = format!("{base_url}/accounts/{requested_account_id}/user-settings");
    let response = send_json(
        client,
        Method::POST,
        &url,
        &request,
        "POST /accounts/{id}/user-settings",
    )
    .await?;
    let status = response.status();
    let json: Value = response
        .json()
        .await
        .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
    if !status.is_success() {
        return Err(failure_from_body(&json));
    }
    serde_json::from_value(json)
        .map_err(|e| HarnessError::Transport(format!("decode upsert response: {e}")))
}

pub(super) async fn list_user_credentials(
    base_url: &str,
    client: &Client,
    requested_account_id: AccountId,
) -> HarnessResult<ListUserCredentialsResponse> {
    let url = format!("{base_url}/accounts/{requested_account_id}/user-credentials");
    let response = send_get_with_retry(client, &url, "GET /accounts/{id}/user-credentials").await?;
    let status = response.status();
    let json: Value = response
        .json()
        .await
        .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
    if !status.is_success() {
        return Err(failure_from_body(&json));
    }
    serde_json::from_value(json)
        .map_err(|e| HarnessError::Transport(format!("decode credentials response: {e}")))
}

pub(super) async fn add_user_credential(
    base_url: &str,
    client: &Client,
    requested_account_id: AccountId,
    mut request: CreateUserCredentialRequest,
) -> HarnessResult<CreateUserCredentialResponse> {
    request.owner_scope = OwnerScope::User {
        account_id: requested_account_id,
    };
    let body = serde_json::json!({
        "kind": request.kind,
        "owner_scope": request.owner_scope,
        "value": request.value.expose_secret(),
    });
    let url = format!("{base_url}/accounts/{requested_account_id}/user-credentials");
    let response = send_json(
        client,
        Method::POST,
        &url,
        &body,
        "POST /accounts/{id}/user-credentials",
    )
    .await?;
    let status = response.status();
    let json: Value = response
        .json()
        .await
        .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
    if !status.is_success() {
        return Err(failure_from_body(&json));
    }
    serde_json::from_value(json)
        .map_err(|e| HarnessError::Transport(format!("decode add credential response: {e}")))
}

pub(super) async fn remove_user_credential(
    base_url: &str,
    client: &Client,
    requested_account_id: AccountId,
    item_id: &str,
) -> HarnessResult<RemoveUserCredentialResponse> {
    let url = format!("{base_url}/accounts/{requested_account_id}/user-credentials/{item_id}");
    let response = send(
        client,
        Method::DELETE,
        &url,
        "DELETE /accounts/{id}/user-credentials/{item_id}",
    )
    .await?;
    let status = response.status();
    let json: Value = response
        .json()
        .await
        .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
    if !status.is_success() {
        return Err(failure_from_body(&json));
    }
    serde_json::from_value(json)
        .map_err(|e| HarnessError::Transport(format!("decode remove credential response: {e}")))
}
