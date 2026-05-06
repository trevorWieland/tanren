//! MCP project tool shared helper functions.
//!
//! Parameter types for list/specs/dependencies tools are re-exported from
//! `tanren_contract` so every interface shares the same shapes. The MCP
//! derives the [`ActorContext`] from its stored authenticated account
//! state — tool parameters do not carry authority.
//!
//! The `success` and `map_failure` helpers are relocated from `lib.rs` so
//! both account and project tools can share them without growing `lib.rs`
//! past the workspace 500-line budget.

use rmcp::model::{CallToolResult, Content};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tanren_app_services::AppServiceError;
use tanren_contract::ProjectFailureBody;

pub(crate) use tanren_contract::ListProjectsParams;

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub(crate) struct ProjectIdParams {
    pub project_id: tanren_identity_policy::ProjectId,
    #[serde(default)]
    pub actor_account_id: Option<tanren_identity_policy::AccountId>,
}

pub(crate) fn success<T: Serialize>(value: &T) -> CallToolResult {
    let text = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_owned());
    CallToolResult::success(vec![Content::text(text)])
}

pub(crate) fn map_failure(err: AppServiceError) -> CallToolResult {
    let body = match err {
        AppServiceError::Project(reason) => {
            serde_json::to_value(ProjectFailureBody::from_reason(reason))
                .unwrap_or_else(|_| json!({}))
        }
        AppServiceError::Account(reason) => {
            json!({"code": reason.code(), "summary": reason.summary()})
        }
        AppServiceError::InvalidInput(message) => {
            json!({"code": "validation_failed", "summary": message})
        }
        AppServiceError::Store(store_err) => {
            json!({"code": "internal_error", "summary": format!("Tanren encountered an internal error: {store_err}")})
        }
        _ => {
            json!({"code": "internal_error", "summary": "Unknown app-service failure"})
        }
    };
    let text = serde_json::to_string(&body).unwrap_or_else(|_| "{}".to_owned());
    CallToolResult::error(vec![Content::text(text)])
}
