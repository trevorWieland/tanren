//! CLI stdout/stderr parsing helpers extracted from `cli.rs` to keep
//! the harness file under the workspace line-count budget.

use std::path::PathBuf;
use std::process::Output;

use chrono::Utc;
use regex::Regex;
use tanren_configuration_secrets::{
    CredentialId, CredentialKind, CredentialScope, UserSettingKey, UserSettingValue,
};
use tanren_contract::AccountView;
use tanren_identity_policy::{AccountId, Identifier, OrgId};
use uuid::Uuid;

use super::shared::{code_to_config_reason, code_to_reason};
use super::{HarnessConfigEntry, HarnessCredential, HarnessError, HarnessResult};

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

pub(crate) fn translate_cli_error(stderr: &[u8]) -> HarnessError {
    let text = String::from_utf8_lossy(stderr);
    let re = Regex::new(r"error:\s*([a-z_]+)\s*—\s*(.*)").expect("constant regex");
    if let Some(captures) = re.captures(&text) {
        let code = captures.get(1).map_or("", |m| m.as_str());
        let summary = captures.get(2).map_or("", |m| m.as_str()).trim().to_owned();
        if let Some(reason) = code_to_config_reason(code) {
            return HarnessError::Configuration(reason, summary);
        }
        if let Some(reason) = code_to_reason(code) {
            return HarnessError::Account(reason, summary);
        }
    }
    HarnessError::Transport(text.into_owned())
}

pub(crate) fn parse_session(
    stdout: &str,
    email: &str,
    display_name: &str,
) -> HarnessResult<(AccountView, bool)> {
    let re = Regex::new(r"account_id=([0-9a-fA-F-]+)\s+session=([^\s]+)").expect("constant regex");
    let captures = re
        .captures(stdout)
        .ok_or_else(|| HarnessError::Transport(format!("could not parse cli stdout: {stdout}")))?;
    let id_raw = captures.get(1).map_or("", |m| m.as_str());
    let token = captures.get(2).map_or("", |m| m.as_str());
    let id = AccountId::from(
        Uuid::parse_str(id_raw)
            .map_err(|e| HarnessError::Transport(format!("parse account id: {e}")))?,
    );
    let identifier = Identifier::from_email(
        &tanren_identity_policy::Email::parse(email)
            .map_err(|e| HarnessError::Transport(format!("parse email: {e}")))?,
    );
    let account = AccountView {
        id,
        identifier,
        display_name: if display_name.is_empty() {
            String::new()
        } else {
            display_name.to_owned()
        },
        org: None,
    };
    Ok((account, !token.is_empty()))
}

pub(crate) fn parse_joined_org(stdout: &str) -> HarnessResult<OrgId> {
    let re = Regex::new(r"joined_org=([0-9a-fA-F-]+)").expect("constant regex");
    let captures = re.captures(stdout).ok_or_else(|| {
        HarnessError::Transport(format!(
            "could not parse joined_org from cli stdout: {stdout}"
        ))
    })?;
    let raw = captures.get(1).map_or("", |m| m.as_str());
    Ok(OrgId::from(Uuid::parse_str(raw).map_err(|e| {
        HarnessError::Transport(format!("parse org id: {e}"))
    })?))
}

pub(crate) fn extract_session_token_from_stdout(stdout: &str) -> String {
    let re = Regex::new(r"session=([^\s]+)").expect("constant regex");
    re.captures(stdout)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_owned()))
        .unwrap_or_default()
}

pub(crate) fn parse_config_entry_stdout(output: &Output) -> HarnessResult<HarnessConfigEntry> {
    let text = String::from_utf8_lossy(&output.stdout);
    let key_re = Regex::new(r"key=([^\s]+)").expect("constant regex");
    let value_re = Regex::new(r"value=([^\s]+)").expect("constant regex");
    let at_re = Regex::new(r"updated_at=([^\s]+)").expect("constant regex");
    let key_str = key_re
        .captures(&text)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_owned()))
        .ok_or_else(|| HarnessError::Transport(format!("parse key from: {text}")))?;
    let value_str = value_re
        .captures(&text)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_owned()))
        .ok_or_else(|| HarnessError::Transport(format!("parse value from: {text}")))?;
    let at_str = at_re
        .captures(&text)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_owned()))
        .ok_or_else(|| HarnessError::Transport(format!("parse updated_at from: {text}")))?;
    let key: UserSettingKey = serde_json::from_value(serde_json::Value::String(key_str))
        .map_err(|e| HarnessError::Transport(format!("parse setting key: {e}")))?;
    let value = UserSettingValue::parse(&value_str)
        .map_err(|e| HarnessError::Transport(format!("parse setting value: {e}")))?;
    let updated_at = chrono::DateTime::parse_from_rfc3339(&at_str)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| HarnessError::Transport(format!("parse updated_at: {e}")))?;
    Ok(HarnessConfigEntry {
        key,
        value,
        updated_at,
    })
}

pub(crate) fn parse_config_list_stdout(output: &Output) -> HarnessResult<Vec<HarnessConfigEntry>> {
    let text = String::from_utf8_lossy(&output.stdout);
    let key_re = Regex::new(r"key=([^\s]+)").expect("constant regex");
    let value_re = Regex::new(r"value=([^\s]+)").expect("constant regex");
    let at_re = Regex::new(r"updated_at=([^\s]+)").expect("constant regex");
    let mut entries = Vec::new();
    for line in text.lines() {
        if line.contains("key=") && line.contains("value=") {
            if let (Some(kc), Some(vc), Some(ac)) = (
                key_re.captures(line),
                value_re.captures(line),
                at_re.captures(line),
            ) {
                let key: UserSettingKey = serde_json::from_value(serde_json::Value::String(
                    kc.get(1).map_or(String::new(), |m| m.as_str().to_owned()),
                ))
                .map_err(|e| HarnessError::Transport(format!("parse key: {e}")))?;
                let value = UserSettingValue::parse(vc.get(1).map_or("", |m| m.as_str()))
                    .map_err(|e| HarnessError::Transport(format!("parse value: {e}")))?;
                let updated_at =
                    chrono::DateTime::parse_from_rfc3339(ac.get(1).map_or("", |m| m.as_str()))
                        .map(|d| d.with_timezone(&Utc))
                        .map_err(|e| HarnessError::Transport(format!("parse updated_at: {e}")))?;
                entries.push(HarnessConfigEntry {
                    key,
                    value,
                    updated_at,
                });
            }
        }
    }
    Ok(entries)
}

pub(crate) fn parse_credential_stdout(output: &Output) -> HarnessResult<HarnessCredential> {
    let text = String::from_utf8_lossy(&output.stdout);
    let id_re = Regex::new(r"id=([0-9a-fA-F-]+)").expect("constant regex");
    let name_re = Regex::new(r"name=([^\s]+)").expect("constant regex");
    let kind_re = Regex::new(r"kind=([^\s]+)").expect("constant regex");
    let scope_re = Regex::new(r"scope=([^\s]+)").expect("constant regex");
    let present_re = Regex::new(r"present=(true|false)").expect("constant regex");
    let id: CredentialId = CredentialId::new(
        Uuid::parse_str(
            id_re
                .captures(&text)
                .and_then(|c| c.get(1).map(|m| m.as_str()))
                .ok_or_else(|| HarnessError::Transport(format!("parse cred id from: {text}")))?,
        )
        .map_err(|e| HarnessError::Transport(format!("parse uuid: {e}")))?,
    );
    let name = name_re
        .captures(&text)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_owned()))
        .ok_or_else(|| HarnessError::Transport("missing name".to_owned()))?;
    let kind: CredentialKind = serde_json::from_value(serde_json::Value::String(
        kind_re
            .captures(&text)
            .and_then(|c| c.get(1).map(|m| m.as_str().to_owned()))
            .ok_or_else(|| HarnessError::Transport("missing kind".to_owned()))?,
    ))
    .map_err(|e| HarnessError::Transport(format!("parse kind: {e}")))?;
    let scope: CredentialScope = serde_json::from_value(serde_json::Value::String(
        scope_re
            .captures(&text)
            .and_then(|c| c.get(1).map(|m| m.as_str().to_owned()))
            .ok_or_else(|| HarnessError::Transport("missing scope".to_owned()))?,
    ))
    .map_err(|e| HarnessError::Transport(format!("parse scope: {e}")))?;
    let present = present_re
        .captures(&text)
        .and_then(|c| c.get(1).map(|m| m.as_str() == "true"))
        .ok_or_else(|| HarnessError::Transport("missing present".to_owned()))?;
    Ok(HarnessCredential {
        id,
        name,
        kind,
        scope,
        present,
    })
}

pub(crate) fn parse_credential_list_stdout(
    output: &Output,
) -> HarnessResult<Vec<HarnessCredential>> {
    let text = String::from_utf8_lossy(&output.stdout);
    let id_re = Regex::new(r"id=([0-9a-fA-F-]+)").expect("constant regex");
    let name_re = Regex::new(r"name=([^\s]+)").expect("constant regex");
    let kind_re = Regex::new(r"kind=([^\s]+)").expect("constant regex");
    let scope_re = Regex::new(r"scope=([^\s]+)").expect("constant regex");
    let present_re = Regex::new(r"present=(true|false)").expect("constant regex");
    let mut creds = Vec::new();
    for line in text.lines() {
        if line.contains("id=") && line.contains("kind=") {
            if let (Some(ic), Some(nc), Some(kc), Some(sc), Some(pc)) = (
                id_re.captures(line),
                name_re.captures(line),
                kind_re.captures(line),
                scope_re.captures(line),
                present_re.captures(line),
            ) {
                let id = CredentialId::new(
                    Uuid::parse_str(ic.get(1).map_or("", |m| m.as_str()))
                        .map_err(|e| HarnessError::Transport(format!("parse uuid: {e}")))?,
                );
                let name = nc.get(1).map_or(String::new(), |m| m.as_str().to_owned());
                let kind: CredentialKind = serde_json::from_value(serde_json::Value::String(
                    kc.get(1).map_or(String::new(), |m| m.as_str().to_owned()),
                ))
                .map_err(|e| HarnessError::Transport(format!("parse kind: {e}")))?;
                let scope: CredentialScope = serde_json::from_value(serde_json::Value::String(
                    sc.get(1).map_or(String::new(), |m| m.as_str().to_owned()),
                ))
                .map_err(|e| HarnessError::Transport(format!("parse scope: {e}")))?;
                let present = pc.get(1).is_some_and(|m| m.as_str() == "true");
                creds.push(HarnessCredential {
                    id,
                    name,
                    kind,
                    scope,
                    present,
                });
            }
        }
    }
    Ok(creds)
}
