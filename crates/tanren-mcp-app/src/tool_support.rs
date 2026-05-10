use rmcp::ErrorData as McpError;
use rmcp::RoleServer;
use rmcp::model::{CallToolResult, Content, ServerCapabilities, ServerInfo};
use rmcp::service::RequestContext;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tanren_app_services::AppServiceError;
use tanren_contract::{
    ListUserCredentialsRequest, ListUserSettingsRequest, UserCredentialId, UserCredentialKind,
    UserSettingKey, UserSettingValue, user_credentials_page_request, user_settings_page_request,
};
use tanren_identity_policy::AccountId;
use uuid::Uuid;

use crate::auth::{ActorCapabilityModel, AuthenticatedPrincipal};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct AccountScopeParams {
    pub(crate) account_id: String,
    pub(crate) limit: Option<u16>,
    pub(crate) after: Option<String>,
}

impl AccountScopeParams {
    #[must_use]
    pub(crate) fn settings_page_request(&self) -> ListUserSettingsRequest {
        user_settings_page_request(self.limit, self.after.clone())
    }

    #[must_use]
    pub(crate) fn credentials_page_request(&self) -> ListUserCredentialsRequest {
        user_credentials_page_request(self.limit, self.after.clone())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct SetUserConfigParams {
    pub(crate) account_id: String,
    pub(crate) key: UserSettingKey,
    pub(crate) value: UserSettingValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct RemoveUserConfigParams {
    pub(crate) account_id: String,
    pub(crate) key: UserSettingKey,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct AddCredentialParams {
    pub(crate) account_id: String,
    pub(crate) kind: UserCredentialKind,
    #[serde(deserialize_with = "tanren_identity_policy::secret_serde::deserialize_password")]
    #[schemars(with = "String")]
    pub(crate) value: secrecy::SecretString,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct UpdateCredentialParams {
    pub(crate) account_id: String,
    pub(crate) item_id: String,
    #[serde(deserialize_with = "tanren_identity_policy::secret_serde::deserialize_password")]
    #[schemars(with = "String")]
    pub(crate) value: secrecy::SecretString,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct RemoveCredentialParams {
    pub(crate) account_id: String,
    pub(crate) item_id: String,
}

/// Encode a successful handler response as a JSON-text `CallToolResult`.
pub(crate) fn success<T: Serialize>(value: &T) -> CallToolResult {
    let text = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_owned());
    CallToolResult::success(vec![Content::text(text)])
}

/// Encode an [`AppServiceError`] as the shared `{code, summary}` error
/// body and surface it as an MCP tool failure result.
pub(crate) fn map_failure(err: AppServiceError) -> CallToolResult {
    let (code, summary) = match err {
        AppServiceError::Account(reason) => (reason.code().to_owned(), reason.summary().to_owned()),
        AppServiceError::Configuration(reason) => {
            (reason.code().to_owned(), reason.summary().to_owned())
        }
        AppServiceError::InvalidInput(message) => ("validation_failed".to_owned(), message),
        AppServiceError::Store(_) => (
            "internal_error".to_owned(),
            "Tanren encountered an internal error.".to_owned(),
        ),
        _ => (
            "internal_error".to_owned(),
            "Unknown app-service failure".to_owned(),
        ),
    };
    let body = json!({
        "code": code,
        "summary": summary,
    });
    let text = serde_json::to_string(&body).unwrap_or_else(|_| "{}".to_owned());
    CallToolResult::error(vec![Content::text(text)])
}

pub(crate) fn validation_failure(summary: &str) -> CallToolResult {
    let body = json!({
        "code": "validation_failed",
        "summary": summary,
    });
    let text = serde_json::to_string(&body).unwrap_or_else(|_| "{}".to_owned());
    CallToolResult::error(vec![Content::text(text)])
}

pub(crate) fn permission_denied_failure(summary: &str) -> CallToolResult {
    let body = json!({
        "code": "permission_denied",
        "summary": summary,
    });
    let text = serde_json::to_string(&body).unwrap_or_else(|_| "{}".to_owned());
    CallToolResult::error(vec![Content::text(text)])
}

pub(crate) fn actor_capability_model_from_context(
    context: &RequestContext<RoleServer>,
) -> Result<ActorCapabilityModel, McpError> {
    principal_from_context(context).map(AuthenticatedPrincipal::capability_model)
}

pub(crate) fn account_principal_from_parts(
    parts: &axum::http::request::Parts,
) -> Result<AccountId, CallToolResult> {
    let principal = principal_from_parts(parts).map_err(|_| {
        permission_denied_failure("An authenticated account session is required for this tool.")
    })?;
    principal.account_id().ok_or_else(|| {
        permission_denied_failure("An authenticated account session is required for this tool.")
    })
}

pub(crate) fn server_info_for_capability_model(
    capability_model: ActorCapabilityModel,
) -> ServerInfo {
    let capabilities = if capability_model.any_tools() {
        ServerCapabilities::builder().enable_tools().build()
    } else {
        ServerCapabilities::builder().build()
    };
    let mut info = ServerInfo::new(capabilities);
    info.instructions = Some(
        "Tanren control plane MCP server. Account-flow tools route through the same handlers the HTTP API uses; failure responses share the {code, summary} error taxonomy."
            .to_owned(),
    );
    info
}

pub(crate) fn parse_account_id(raw: &str) -> Result<AccountId, String> {
    let parsed = Uuid::parse_str(raw).map_err(|_| "account_id must be a valid uuid".to_owned())?;
    Ok(AccountId::new(parsed))
}

pub(crate) fn parse_user_credential_id(raw: &str) -> Result<UserCredentialId, String> {
    UserCredentialId::parse(raw).map_err(|_| "item_id must be a valid uuid".to_owned())
}

fn principal_from_context(
    context: &RequestContext<RoleServer>,
) -> Result<AuthenticatedPrincipal, McpError> {
    let Some(parts) = context.extensions.get::<axum::http::request::Parts>() else {
        return Err(McpError::invalid_params(
            "missing HTTP request context in MCP tool call",
            None,
        ));
    };
    principal_from_parts(parts)
}

fn principal_from_parts(
    parts: &axum::http::request::Parts,
) -> Result<AuthenticatedPrincipal, McpError> {
    parts
        .extensions
        .get::<AuthenticatedPrincipal>()
        .copied()
        .ok_or_else(|| McpError::invalid_params("missing authenticated MCP principal", None))
}
