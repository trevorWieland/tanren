use rmcp::model::{CallToolResult, Content};
use schemars::JsonSchema;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tanren_app_services::AppServiceError;
use tanren_configuration_secrets::{
    CredentialId, CredentialKind, UserSettingKey, UserSettingValue,
};
use tanren_contract::{
    EvaluateNotificationRouteRequest, SetNotificationPreferencesRequest,
    SetOrganizationNotificationOverridesRequest,
};
use tanren_identity_policy::secret_serde;

#[derive(serde::Serialize)]
pub(crate) struct HealthResponse {
    pub status: String,
    pub version: String,
    pub contract_version: u32,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct SessionParams {
    pub session_token: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct SetUserConfigParams {
    pub session_token: String,
    pub key: UserSettingKey,
    pub value: UserSettingValue,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct RemoveUserConfigParams {
    pub session_token: String,
    pub key: UserSettingKey,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct CreateCredentialParams {
    pub session_token: String,
    pub kind: CredentialKind,
    pub name: String,
    pub description: Option<String>,
    pub provider: Option<String>,
    #[schemars(with = "String")]
    #[serde(deserialize_with = "secret_serde::deserialize_password")]
    pub value: SecretString,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct UpdateCredentialParams {
    pub session_token: String,
    pub id: CredentialId,
    pub name: Option<String>,
    pub description: Option<String>,
    #[schemars(with = "String")]
    #[serde(deserialize_with = "secret_serde::deserialize_password")]
    pub value: SecretString,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct RemoveCredentialParams {
    pub session_token: String,
    pub id: CredentialId,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct SetNotificationPreferencesParams {
    pub session_token: String,
    pub request: SetNotificationPreferencesRequest,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct SetOrganizationNotificationOverridesParams {
    pub session_token: String,
    pub request: SetOrganizationNotificationOverridesRequest,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct EvaluateNotificationRouteParams {
    pub session_token: String,
    pub request: EvaluateNotificationRouteRequest,
}

pub(crate) fn success<T: Serialize>(value: &T) -> CallToolResult {
    let text = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_owned());
    CallToolResult::success(vec![Content::text(text)])
}

pub(crate) fn map_failure(err: AppServiceError) -> CallToolResult {
    let (code, summary) = match err {
        AppServiceError::Account(reason) => (reason.code().to_owned(), reason.summary().to_owned()),
        AppServiceError::Configuration(reason) => {
            (reason.code().to_owned(), reason.summary().to_owned())
        }
        AppServiceError::InvalidInput(message) => ("validation_failed".to_owned(), message),
        AppServiceError::Store(err) => (
            "internal_error".to_owned(),
            format!("Tanren encountered an internal error: {err}"),
        ),
        _ => (
            "internal_error".to_owned(),
            "Unknown app-service failure".to_owned(),
        ),
    };
    let body = serde_json::json!({ "code": code, "summary": summary });
    let text = serde_json::to_string(&body).unwrap_or_else(|_| "{}".to_owned());
    CallToolResult::error(vec![Content::text(text)])
}
