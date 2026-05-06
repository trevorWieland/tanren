//! Shared helpers for the `@api` and `@cli`/`@mcp` wire harnesses.
//!
//! Extracted from `api.rs` to keep that file under the line budget.
//! Functions marked `pub(crate)` are used by multiple harness modules;
//! those marked `pub(super)` are internal to the `harness` module.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use secrecy::ExposeSecret;
use serde_json::Value;
use tanren_contract::{
    AcceptInvitationRequest, AccountFailureReason, AccountView, ProjectFailureReason,
    SignInRequest, SignUpRequest,
};

use super::{HarnessError, HarnessResult, HarnessSession};

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

pub(super) fn failure_from_body(json: &Value) -> HarnessError {
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
    } else if let Some(reason) = code_to_project_reason(&code) {
        HarnessError::Project(reason, summary)
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

pub(crate) fn code_to_project_reason(code: &str) -> Option<ProjectFailureReason> {
    Some(match code {
        "unauthorized_project_access" => ProjectFailureReason::UnauthorizedProjectAccess,
        "unknown_project" => ProjectFailureReason::UnknownProject,
        "unknown_spec" => ProjectFailureReason::UnknownSpec,
        _ => return None,
    })
}

pub(crate) fn locate_workspace_binary(name: &str) -> HarnessResult<PathBuf> {
    if let Ok(explicit) = std::env::var(format!(
        "TANREN_BIN_{}",
        name.replace('-', "_").to_uppercase()
    )) {
        let p = PathBuf::from(explicit);
        if p.exists() {
            return Ok(p);
        }
    }
    let exe = std::env::current_exe()
        .map_err(|e| HarnessError::Transport(format!("current exe: {e}")))?;
    let dir = exe
        .parent()
        .ok_or_else(|| HarnessError::Transport("current exe has no parent".to_owned()))?;
    let mut candidate = dir.join(name);
    if cfg!(windows) {
        candidate.set_extension("exe");
    }
    if candidate.exists() {
        return Ok(candidate);
    }
    let mut cursor = dir;
    while let Some(parent) = cursor.parent() {
        for profile in ["debug", "release"] {
            let mut probe = parent.join("target").join(profile).join(name);
            if cfg!(windows) {
                probe.set_extension("exe");
            }
            if probe.exists() {
                return Ok(probe);
            }
        }
        cursor = parent;
    }
    Err(HarnessError::Transport(format!(
        "binary `{name}` not found alongside test executable {} — run `cargo build --workspace`",
        exe.display()
    )))
}

pub(super) fn has_session_cookie(headers: &reqwest::header::HeaderMap) -> bool {
    headers
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .any(|v| {
            v.to_str()
                .ok()
                .is_some_and(|s| s.starts_with("tanren_session="))
        })
}

pub(super) fn extract_account_and_expiry(
    json: &Value,
) -> HarnessResult<(AccountView, DateTime<Utc>)> {
    let account: AccountView = serde_json::from_value(json["account"].clone())
        .map_err(|e| HarnessError::Transport(format!("decode account: {e}")))?;
    let expires_at = json["session"]["expires_at"]
        .as_str()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc))
        .ok_or_else(|| HarnessError::Transport("missing session.expires_at".to_owned()))?;
    Ok((account, expires_at))
}

pub(super) fn session_from_parts(
    account: AccountView,
    expires_at: DateTime<Utc>,
    has_token: bool,
) -> HarnessSession {
    HarnessSession {
        account_id: account.id,
        account,
        expires_at,
        has_token,
    }
}
