use chrono::Utc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_app_services::{AppServiceError, OrganizationStore, Store};
use tanren_contract::{OrganizationAdminOperation, OrganizationFailureReason};
use tanren_identity_policy::{AccountId, OrgId, OrganizationName};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct OrgCreateReq {
    #[serde(default)]
    pub(crate) session_token: Option<String>,
    pub(crate) name: OrganizationName,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct OrgListReq {
    #[serde(default)]
    pub(crate) session_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct OrgAuthReq {
    #[serde(default)]
    pub(crate) session_token: Option<String>,
    pub(crate) org_id: OrgId,
    pub(crate) operation: OrganizationAdminOperation,
}

pub(crate) fn require_token(token: Option<&String>) -> Result<&str, AppServiceError> {
    token
        .map(String::as_str)
        .filter(|s| !s.is_empty())
        .ok_or(AppServiceError::Organization(
            OrganizationFailureReason::AuthRequired,
        ))
}

pub(crate) async fn resolve_session(
    store: &Store,
    token: &str,
) -> Result<AccountId, AppServiceError> {
    let now = Utc::now();
    let session = store.resolve_bearer_session(token, now).await?;
    match session {
        Some(s) => Ok(s.account_id),
        None => Err(AppServiceError::Organization(
            OrganizationFailureReason::AuthRequired,
        )),
    }
}
