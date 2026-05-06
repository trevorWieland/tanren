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

pub(crate) fn sign_up_body(req: &SignUpRequest) -> Value {
    serde_json::json!({
        "email": req.email.as_str(),
        "password": req.password.expose_secret(),
        "display_name": req.display_name,
    })
}

pub(crate) fn sign_in_body(req: &SignInRequest) -> Value {
    serde_json::json!({
        "email": req.email.as_str(),
        "password": req.password.expose_secret(),
    })
}

pub(crate) fn accept_invitation_body(req: &AcceptInvitationRequest) -> Value {
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
        .unwrap_or("transport_error")
        .to_owned();
    let summary = json
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("unknown failure")
        .to_owned();
    if let Some(reason) = code_to_reason(&code) {
        HarnessError::Account(reason, summary)
    } else if let Some(reason) = code_to_org_reason(&code) {
        HarnessError::Organization(reason, summary)
    } else {
        HarnessError::Transport(format!("{code}: {summary}"))
    }
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

pub(crate) fn code_to_org_reason(code: &str) -> Option<OrganizationFailureReason> {
    Some(match code {
        "auth_required" => OrganizationFailureReason::AuthRequired,
        "permission_denied" => OrganizationFailureReason::PermissionDenied,
        "duplicate_organization_name" => OrganizationFailureReason::DuplicateOrganizationName,
        "last_admin_holder" => OrganizationFailureReason::LastAdminHolder,
        "not_found" => OrganizationFailureReason::NotFound,
        _ => return None,
    })
}
