use std::path::{Path, PathBuf};

use reqwest::header::HeaderMap;
use serde_json::Value;
use tanren_contract::{
    AcceptInvitationRequest, AccountFailureReason, DeploymentPostureScope, SignInRequest,
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

pub(crate) fn sqlite_url(path: &Path) -> String {
    format!("sqlite://{}?mode=rwc", path.display())
}

pub(crate) fn sign_up_body(req: &SignUpRequest) -> Value {
    use secrecy::ExposeSecret;
    serde_json::json!({
        "email": req.email.as_str(),
        "password": req.password.expose_secret(),
        "display_name": req.display_name,
    })
}

pub(crate) fn sign_in_body(req: &SignInRequest) -> Value {
    use secrecy::ExposeSecret;
    serde_json::json!({
        "email": req.email.as_str(),
        "password": req.password.expose_secret(),
    })
}

pub(crate) fn accept_invitation_body(req: &AcceptInvitationRequest) -> Value {
    use secrecy::ExposeSecret;
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
    } else if code != "transport_error" {
        HarnessError::FailureCode { code, summary }
    } else {
        HarnessError::Transport(format!("{code}: {summary}"))
    }
}

pub(crate) fn has_session_cookie(headers: &HeaderMap) -> bool {
    headers
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .any(|cookie| {
            cookie
                .split(';')
                .next()
                .and_then(|pair| pair.split_once('='))
                .is_some_and(|(name, value)| name.trim() == "tanren_session" && !value.is_empty())
        })
}

pub(crate) fn has_session_bearer_token(json: &Value) -> bool {
    json.get("session")
        .and_then(|session| session.get("token"))
        .and_then(Value::as_str)
        .is_some_and(|token| !token.trim().is_empty())
}

pub(crate) fn scope_path(scope: DeploymentPostureScope) -> (&'static str, String) {
    match scope {
        DeploymentPostureScope::Account { account_id } => ("account", account_id.to_string()),
        DeploymentPostureScope::Project { project_id } => ("project", project_id.to_string()),
        DeploymentPostureScope::Installation { installation_id } => {
            ("installation", installation_id.to_string())
        }
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
