//! Shared wire-harness support used by `api`, `cli`, and `mcp` harness
//! implementations.

use std::path::PathBuf;

use serde_json::Value;
use tanren_contract::AccountFailureReason;

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

pub(crate) fn failure_from_body(json: &Value) -> HarnessError {
    let code = json
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or("transport_error");
    let summary = json
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("unknown failure")
        .to_owned();
    match code_to_reason(code) {
        Some(reason) => HarnessError::Account(reason, summary),
        None => HarnessError::Transport(format!("{code}: {summary}")),
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
        "wrong_account" => AccountFailureReason::WrongAccount,
        "unauthenticated" => AccountFailureReason::Unauthenticated,
        _ => return None,
    })
}
