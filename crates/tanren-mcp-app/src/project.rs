//! MCP project tool parameter types and shared helper functions.
//!
//! Parameter types live here because the rmcp `Parameters` extractor
//! requires `Deserialize + JsonSchema`, and the handler interfaces for
//! list / specs / dependencies don't map 1 : 1 to an existing contract
//! request type.
//!
//! The `success` and `map_failure` helpers are relocated from `lib.rs` so
//! both account and project tools can share them without growing `lib.rs`
//! past the workspace 500-line budget.

use rmcp::model::{CallToolResult, Content};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::json;
use tanren_app_services::AppServiceError;
use tanren_identity_policy::{AccountId, ProjectId};

/// Parameter type for the `project.list` MCP tool.
#[derive(Debug, Clone, serde::Deserialize, JsonSchema)]
pub(crate) struct ListProjectsParams {
    /// Account whose projects to list.
    pub account_id: AccountId,
}

/// Parameter type for the `project.specs` and `project.dependencies` MCP
/// tools.
#[derive(Debug, Clone, serde::Deserialize, JsonSchema)]
pub(crate) struct ProjectIdParams {
    /// Project to query.
    pub project_id: ProjectId,
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
        AppServiceError::Project(reason) => (reason.code().to_owned(), reason.summary().to_owned()),
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
    let body = json!({
        "code": code,
        "summary": summary,
    });
    let text = serde_json::to_string(&body).unwrap_or_else(|_| "{}".to_owned());
    CallToolResult::error(vec![Content::text(text)])
}
