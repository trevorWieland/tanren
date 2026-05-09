use reqwest::Client;
use serde_json::Value;
use tanren_configuration_secrets::OwnerScope;
use tanren_contract::{
    CreateUserCredentialRequest, CreateUserCredentialResponse, ListUserCredentialsResponse,
    ListUserSettingsResponse, RemoveUserCredentialResponse, UpsertUserSettingRequest,
    UpsertUserSettingResponse,
};
use tanren_identity_policy::AccountId;

use super::{HarnessError, HarnessResult, failure_from_body};

pub(super) async fn list_user_settings(
    base_url: &str,
    client: &Client,
    requested_account_id: AccountId,
) -> HarnessResult<ListUserSettingsResponse> {
    let url = format!("{base_url}/accounts/{requested_account_id}/user-settings");
    let response =
        client.get(&url).send().await.map_err(|e| {
            HarnessError::Transport(format!("GET /accounts/{{id}}/user-settings: {e}"))
        })?;
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
    let response = client.post(&url).json(&request).send().await.map_err(|e| {
        HarnessError::Transport(format!("POST /accounts/{{id}}/user-settings: {e}"))
    })?;
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
    let response = client.get(&url).send().await.map_err(|e| {
        HarnessError::Transport(format!("GET /accounts/{{id}}/user-credentials: {e}"))
    })?;
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
    let url = format!("{base_url}/accounts/{requested_account_id}/user-credentials");
    let response = client.post(&url).json(&request).send().await.map_err(|e| {
        HarnessError::Transport(format!("POST /accounts/{{id}}/user-credentials: {e}"))
    })?;
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
    let response = client.delete(&url).send().await.map_err(|e| {
        HarnessError::Transport(format!(
            "DELETE /accounts/{{id}}/user-credentials/{{item_id}}: {e}"
        ))
    })?;
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
