//! Shared helper functions for the `@api` and `@cli`/`@mcp` wire harnesses.
//!
//! Extracted from `api.rs` to keep the harness implementation files under
//! the workspace 500-line budget.

use std::path::PathBuf;

use secrecy::ExposeSecret;
use serde_json::Value;
use tanren_contract::{
    AcceptInvitationRequest, AccountFailureReason, OrganizationFailureReason, SignInRequest,
    SignUpRequest,
};

use super::HarnessError;

pub(crate) fn scenario_db_path(prefix: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "tanren-bdd-{prefix}-{}-{}.db",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    p
}

pub(crate) fn sqlite_url(path: &std::path::Path) -> String {
    format!("sqlite://{}?mode=rwc", path.display())
}

pub(super) fn sign_up_body(req: &SignUpRequest) -> Value {
    serde_json::json!({
        "email": req.email.as_str(),
        "password": req.password.expose_secret(),
        "display_name": req.display_name,
    })
}

pub(super) fn sign_in_body(req: &SignInRequest) -> Value {
    serde_json::json!({
        "email": req.email.as_str(),
        "password": req.password.expose_secret(),
    })
}

pub(super) fn accept_invitation_body(req: &AcceptInvitationRequest) -> Value {
    serde_json::json!({
        "email": req.email.as_str(),
        "password": req.password.expose_secret(),
        "display_name": req.display_name,
    })
}

pub(crate) fn failure_from_body(json: &Value) -> HarnessError {
    let code = json
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or("transport_error");
    let summary = json
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("unknown failure");
    if let Some(reason) = code_to_reason(code) {
        return HarnessError::Account(reason, summary.to_owned());
    }
    if let Some(reason) = org_code_to_reason(code) {
        return HarnessError::Organization(reason, summary.to_owned());
    }
    HarnessError::Transport(format!("{code}: {summary}"))
}

pub(crate) fn code_to_reason(code: &str) -> Option<AccountFailureReason> {
    Some(match code {
        "duplicate_identifier" => AccountFailureReason::DuplicateIdentifier,
        "invalid_credential" => AccountFailureReason::InvalidCredential,
        "validation_failed" => AccountFailureReason::ValidationFailed,
        "invitation_not_found" => AccountFailureReason::InvitationNotFound,
        "invitation_expired" => AccountFailureReason::InvitationExpired,
        "invitation_already_consumed" => AccountFailureReason::InvitationAlreadyConsumed,
        _ => return None,
    })
}

pub(crate) fn org_code_to_reason(code: &str) -> Option<OrganizationFailureReason> {
    Some(match code {
        "unauthenticated" => OrganizationFailureReason::Unauthenticated,
        "duplicate_name" => OrganizationFailureReason::DuplicateName,
        "not_authorized" => OrganizationFailureReason::NotAuthorized,
        _ => return None,
    })
}
