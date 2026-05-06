//! Shared wire-harness support used by `api`, `cli`, and `mcp` harness
//! implementations.

use std::path::PathBuf;

use serde_json::Value;
use tanren_contract::AccountFailureReason;
use tanren_store::NewInvitation;

use super::{HarnessError, HarnessInvitation};

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

pub(crate) fn harness_to_new_invitation(fixture: HarnessInvitation) -> NewInvitation {
    NewInvitation {
        token: fixture.token,
        inviting_org_id: fixture.inviting_org,
        expires_at: fixture.expires_at,
        target_identifier: fixture.target_identifier,
        org_permissions: fixture.org_permissions,
        revoked: fixture.revoked,
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

pub(crate) async fn wait_for_http_ready(
    client: &reqwest::Client,
    base_url: &str,
) -> Result<(), HarnessError> {
    let url = format!("{base_url}/health");
    let mut last_err = String::new();
    for _ in 0..500 {
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => return Ok(()),
            Ok(resp) => {
                last_err = format!("health check returned {}", resp.status());
            }
            Err(e) => {
                last_err = e.to_string();
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    Err(HarnessError::Transport(format!(
        "server did not become ready within retry budget: {last_err}"
    )))
}
