//! Shared wire-harness helpers used across the api, cli, and mcp
//! implementations. Extracted from `api.rs` to keep each harness file
//! under the workspace line-count budget.

use std::path::PathBuf;

use reqwest::Client;
use secrecy::ExposeSecret;
use serde_json::Value;
use tanren_configuration_secrets::{CredentialId, CredentialKind, CredentialScope};
use tanren_contract::{
    AcceptInvitationRequest, AccountFailureReason, AccountView, ConfigurationFailureReason,
    SignInRequest, SignUpRequest,
};

use super::{HarnessAcceptance, HarnessCredential, HarnessError, HarnessResult, HarnessSession};

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
    if let Some(reason) = code_to_config_reason(&code) {
        HarnessError::Configuration(reason, summary)
    } else if let Some(reason) = code_to_reason(&code) {
        HarnessError::Account(reason, summary)
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

pub(crate) fn code_to_config_reason(code: &str) -> Option<ConfigurationFailureReason> {
    Some(match code {
        "setting_not_found" => ConfigurationFailureReason::SettingNotFound,
        "invalid_setting_value" => ConfigurationFailureReason::InvalidSettingValue,
        "invalid_setting_key" => ConfigurationFailureReason::InvalidSettingKey,
        "credential_not_found" => ConfigurationFailureReason::CredentialNotFound,
        "duplicate_credential_name" => ConfigurationFailureReason::DuplicateCredentialName,
        "credential_kind_scope_mismatch" => ConfigurationFailureReason::CredentialKindScopeMismatch,
        "validation_failed" => ConfigurationFailureReason::ValidationFailed,
        "unauthorized" => ConfigurationFailureReason::Unauthorized,
        _ => return None,
    })
}

pub(crate) fn extract_session_cookie(response: &reqwest::Response) -> Option<String> {
    response
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .find_map(|v| {
            let s = v.to_str().ok()?;
            let prefix = "tanren_session=";
            let start = s.find(prefix)?;
            let rest = &s[start + prefix.len()..];
            let end = rest.find(';').unwrap_or(rest.len());
            Some(rest[..end].to_owned())
        })
}

pub(crate) fn decode_credential(val: &Value) -> HarnessResult<HarnessCredential> {
    let id: CredentialId = serde_json::from_value(val["id"].clone())
        .map_err(|e| HarnessError::Transport(format!("decode cred id: {e}")))?;
    let name = val["name"]
        .as_str()
        .ok_or_else(|| HarnessError::Transport("missing cred name".to_owned()))?
        .to_owned();
    let kind: CredentialKind = serde_json::from_value(val["kind"].clone())
        .map_err(|e| HarnessError::Transport(format!("decode cred kind: {e}")))?;
    let scope: CredentialScope = serde_json::from_value(val["scope"].clone())
        .map_err(|e| HarnessError::Transport(format!("decode cred scope: {e}")))?;
    let present = val["present"]
        .as_bool()
        .ok_or_else(|| HarnessError::Transport("missing cred present".to_owned()))?;
    Ok(HarnessCredential {
        id,
        name,
        kind,
        scope,
        present,
    })
}

pub(crate) async fn run_concurrent_acceptances(
    base_url: String,
    requests: Vec<AcceptInvitationRequest>,
) -> Vec<HarnessResult<HarnessAcceptance>> {
    let mut handles = Vec::with_capacity(requests.len());
    for req in requests {
        let url = format!(
            "{}/invitations/{}/accept",
            base_url,
            req.invitation_token.as_str()
        );
        let body = accept_invitation_body(&req);
        let client = match Client::builder().build() {
            Ok(c) => c,
            Err(e) => {
                handles.push(tokio::spawn(async move {
                    Err::<HarnessAcceptance, HarnessError>(HarnessError::Transport(format!(
                        "build client: {e}"
                    )))
                }));
                continue;
            }
        };
        handles.push(tokio::spawn(async move {
            let response = client.post(&url).json(&body).send().await.map_err(|e| {
                HarnessError::Transport(format!("POST /invitations/{{token}}/accept: {e}"))
            })?;
            let status = response.status();
            let cookies_set = response
                .headers()
                .get_all(reqwest::header::SET_COOKIE)
                .iter()
                .any(|v| {
                    v.to_str()
                        .ok()
                        .is_some_and(|s| s.starts_with("tanren_session="))
                });
            let json: Value = response
                .json()
                .await
                .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
            if !status.is_success() {
                return Err(failure_from_body(&json));
            }
            let account: AccountView = serde_json::from_value(json["account"].clone())
                .map_err(|e| HarnessError::Transport(format!("decode account: {e}")))?;
            let expires_at = json["session"]["expires_at"]
                .as_str()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|d| d.with_timezone(&chrono::Utc))
                .ok_or_else(|| HarnessError::Transport("missing session.expires_at".to_owned()))?;
            let joined_org = serde_json::from_value(json["joined_org"].clone())
                .map_err(|e| HarnessError::Transport(format!("decode joined_org: {e}")))?;
            Ok(HarnessAcceptance {
                session: HarnessSession {
                    account_id: account.id,
                    account,
                    expires_at,
                    has_token: cookies_set,
                },
                joined_org,
            })
        }));
    }
    let mut out = Vec::with_capacity(handles.len());
    for h in handles {
        out.push(match h.await {
            Ok(r) => r,
            Err(e) => Err(HarnessError::Transport(format!("join: {e}"))),
        });
    }
    out
}
