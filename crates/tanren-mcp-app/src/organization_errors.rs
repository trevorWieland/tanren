use rmcp::model::{CallToolResult, Content};
use tanren_app_services::{AppServiceError, map_organization_error};
use tanren_contract::{AccountFailureReason, OrganizationFailureBody};

pub(super) fn organization_auth_required_failure() -> CallToolResult {
    map_organization_failure(&AppServiceError::Account(
        AccountFailureReason::AuthRequired,
    ))
}

pub(super) fn map_organization_failure(err: &AppServiceError) -> CallToolResult {
    let projection = map_organization_error(err);
    if let AppServiceError::Store(store_err) = &err {
        tracing::error!(target: "tanren_mcp", error = %store_err, "store error");
    }
    let body = OrganizationFailureBody {
        code: projection.body.code,
        summary: projection.body.summary,
    };
    let text = serde_json::to_string(&body).unwrap_or_else(|_| "{}".to_owned());
    CallToolResult::error(vec![Content::text(text)])
}
