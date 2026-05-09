use std::path::Path;
use std::process::Stdio;

use chrono::Utc;
use regex::Regex;
use secrecy::ExposeSecret;
use tanren_configuration_secrets::{
    OwnerScope, ThemePreference, UserCredentialKind, UserCredentialStatus, UserSettingKey,
    UserSettingValue,
};
use tanren_contract::{
    CreateUserCredentialRequest, CreateUserCredentialResponse, ListUserCredentialsResponse,
    ListUserSettingsResponse, RemoveUserCredentialResponse, UpsertUserSettingRequest,
    UpsertUserSettingResponse, UserCredentialView, UserSettingView,
};
use tanren_identity_policy::AccountId;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use uuid::Uuid;

use super::{HarnessError, HarnessResult, translate_cli_error};

pub(super) async fn list_user_settings(
    binary: &Path,
    db_url: &str,
    session_path: &Path,
    requested_account_id: AccountId,
) -> HarnessResult<ListUserSettingsResponse> {
    let output = Command::new(binary)
        .args([
            "config",
            "user",
            "list",
            "--database-url",
            db_url,
            "--account-id",
            &requested_account_id.to_string(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("TANREN_SESSION_FILE", session_path)
        .output()
        .await
        .map_err(|e| HarnessError::Transport(format!("spawn tanren-cli: {e}")))?;
    if !output.status.success() {
        return Err(translate_cli_error(&output.stderr));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_setting_rows(&stdout).map(|items| ListUserSettingsResponse {
        items,
        next_cursor: parse_next_cursor(&stdout),
    })
}

pub(super) async fn upsert_user_setting(
    binary: &Path,
    db_url: &str,
    session_path: &Path,
    requested_account_id: AccountId,
    request: UpsertUserSettingRequest,
) -> HarnessResult<UpsertUserSettingResponse> {
    let key = setting_key_name(request.key);
    let value = setting_value_cli_arg(&request.value);
    let output = Command::new(binary)
        .args([
            "config",
            "user",
            "set",
            "--database-url",
            db_url,
            "--account-id",
            &requested_account_id.to_string(),
            "--key",
            key,
            "--value",
            &value,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("TANREN_SESSION_FILE", session_path)
        .output()
        .await
        .map_err(|e| HarnessError::Transport(format!("spawn tanren-cli: {e}")))?;
    if !output.status.success() {
        return Err(translate_cli_error(&output.stderr));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut items = parse_setting_rows(&stdout)?;
    let setting = items
        .pop()
        .ok_or_else(|| HarnessError::Transport("missing setting row from cli output".to_owned()))?;
    Ok(UpsertUserSettingResponse { setting })
}

pub(super) async fn list_user_credentials(
    binary: &Path,
    db_url: &str,
    session_path: &Path,
    requested_account_id: AccountId,
) -> HarnessResult<ListUserCredentialsResponse> {
    let output = Command::new(binary)
        .args([
            "credential",
            "list",
            "--database-url",
            db_url,
            "--account-id",
            &requested_account_id.to_string(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("TANREN_SESSION_FILE", session_path)
        .output()
        .await
        .map_err(|e| HarnessError::Transport(format!("spawn tanren-cli: {e}")))?;
    if !output.status.success() {
        return Err(translate_cli_error(&output.stderr));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_credential_rows(&stdout).map(|items| ListUserCredentialsResponse {
        items,
        next_cursor: parse_next_cursor(&stdout),
    })
}

pub(super) async fn add_user_credential(
    binary: &Path,
    db_url: &str,
    session_path: &Path,
    requested_account_id: AccountId,
    request: CreateUserCredentialRequest,
) -> HarnessResult<CreateUserCredentialResponse> {
    let kind = credential_kind_name(request.kind);
    let value = request.value.expose_secret().to_owned();
    let mut child = Command::new(binary)
        .args([
            "credential",
            "add",
            "--database-url",
            db_url,
            "--account-id",
            &requested_account_id.to_string(),
            "--kind",
            kind,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("TANREN_SESSION_FILE", session_path)
        .spawn()
        .map_err(|e| HarnessError::Transport(format!("spawn tanren-cli: {e}")))?;
    let mut stdin = child.stdin.take().ok_or_else(|| {
        HarnessError::Transport("tanren-cli stdin was not piped for credential add".to_owned())
    })?;
    stdin
        .write_all(value.as_bytes())
        .await
        .map_err(|e| HarnessError::Transport(format!("write credential value to stdin: {e}")))?;
    stdin
        .write_all(b"\n")
        .await
        .map_err(|e| HarnessError::Transport(format!("terminate credential stdin line: {e}")))?;
    drop(stdin);
    let output = child
        .wait_with_output()
        .await
        .map_err(|e| HarnessError::Transport(format!("wait for tanren-cli: {e}")))?;
    if !output.status.success() {
        return Err(translate_cli_error(&output.stderr));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut items = parse_credential_rows(&stdout)?;
    let item = items.pop().ok_or_else(|| {
        HarnessError::Transport("missing credential row from cli output".to_owned())
    })?;
    Ok(CreateUserCredentialResponse { item })
}

pub(super) async fn remove_user_credential(
    binary: &Path,
    db_url: &str,
    session_path: &Path,
    requested_account_id: AccountId,
    item_id: &str,
) -> HarnessResult<RemoveUserCredentialResponse> {
    let output = Command::new(binary)
        .args([
            "credential",
            "remove",
            "--database-url",
            db_url,
            "--account-id",
            &requested_account_id.to_string(),
            "--item-id",
            item_id,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("TANREN_SESSION_FILE", session_path)
        .output()
        .await
        .map_err(|e| HarnessError::Transport(format!("spawn tanren-cli: {e}")))?;
    if !output.status.success() {
        return Err(translate_cli_error(&output.stderr));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut items = parse_credential_rows(&stdout)?;
    let item = items.pop().ok_or_else(|| {
        HarnessError::Transport("missing removed credential row from cli output".to_owned())
    })?;
    Ok(RemoveUserCredentialResponse { item })
}

fn parse_setting_rows(stdout: &str) -> HarnessResult<Vec<UserSettingView>> {
    let re = Regex::new(r"^(?:setting|removed) key=([a-z_]+) value=([^\s]+) updated_at=([^\s]+)$")
        .expect("constant regex");
    let mut items = Vec::new();
    for line in stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if !(line.starts_with("setting ") || line.starts_with("removed ")) {
            continue;
        }
        let captures = re.captures(line).ok_or_else(|| {
            HarnessError::Transport(format!("could not parse cli setting row: {line}"))
        })?;
        let key = parse_setting_key(captures.get(1).map_or("", |m| m.as_str()))?;
        let value = parse_setting_value(captures.get(2).map_or("", |m| m.as_str()))?;
        let updated_at = parse_rfc3339(captures.get(3).map_or("", |m| m.as_str()))?;
        items.push(UserSettingView {
            key,
            value,
            updated_at,
        });
    }
    Ok(items)
}

fn parse_credential_rows(stdout: &str) -> HarnessResult<Vec<UserCredentialView>> {
    let re = Regex::new(
        r"^(?:credential|removed) id=([^\s]+) kind=([a-z_]+) scope=user:([0-9a-fA-F-]+) status=([a-z_]+) created_at=([^\s]+) updated_at=([^\s]+)$",
    )
    .expect("constant regex");
    let mut items = Vec::new();
    for line in stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if !(line.starts_with("credential ") || line.starts_with("removed ")) {
            continue;
        }
        let captures = re.captures(line).ok_or_else(|| {
            HarnessError::Transport(format!("could not parse cli credential row: {line}"))
        })?;
        let id = captures.get(1).map_or("", |m| m.as_str()).to_owned();
        let kind = parse_credential_kind(captures.get(2).map_or("", |m| m.as_str()))?;
        let owner_account_id = AccountId::new(
            Uuid::parse_str(captures.get(3).map_or("", |m| m.as_str()))
                .map_err(|e| HarnessError::Transport(format!("parse owner account id: {e}")))?,
        );
        let status = parse_credential_status(captures.get(4).map_or("", |m| m.as_str()))?;
        let created_at = parse_rfc3339(captures.get(5).map_or("", |m| m.as_str()))?;
        let updated_at = parse_rfc3339(captures.get(6).map_or("", |m| m.as_str()))?;
        items.push(UserCredentialView {
            id,
            kind,
            owner_scope: OwnerScope::User {
                account_id: owner_account_id,
            },
            status,
            created_at,
            updated_at,
        });
    }
    Ok(items)
}

fn parse_next_cursor(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("next_cursor=").map(str::to_owned))
}

fn parse_rfc3339(raw: &str) -> HarnessResult<chrono::DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(raw)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| HarnessError::Transport(format!("parse timestamp '{raw}': {e}")))
}

fn parse_setting_key(raw: &str) -> HarnessResult<UserSettingKey> {
    match raw {
        "theme" => Ok(UserSettingKey::Theme),
        "editor" => Ok(UserSettingKey::Editor),
        _ => Err(HarnessError::Transport(format!(
            "unknown setting key from cli output: {raw}"
        ))),
    }
}

fn parse_setting_value(raw: &str) -> HarnessResult<UserSettingValue> {
    if let Some(theme) = raw.strip_prefix("theme:") {
        return Ok(UserSettingValue::Theme(match theme {
            "system" => ThemePreference::System,
            "light" => ThemePreference::Light,
            "dark" => ThemePreference::Dark,
            _ => {
                return Err(HarnessError::Transport(format!(
                    "unknown theme value from cli output: {theme}"
                )));
            }
        }));
    }
    if let Some(editor) = raw.strip_prefix("editor:") {
        return Ok(UserSettingValue::Editor(editor.to_owned()));
    }
    Err(HarnessError::Transport(format!(
        "unknown setting value from cli output: {raw}"
    )))
}

fn parse_credential_kind(raw: &str) -> HarnessResult<UserCredentialKind> {
    match raw {
        "provider_api_token" => Ok(UserCredentialKind::ProviderApiToken),
        "harness_api_token" => Ok(UserCredentialKind::HarnessApiToken),
        _ => Err(HarnessError::Transport(format!(
            "unknown credential kind from cli output: {raw}"
        ))),
    }
}

fn parse_credential_status(raw: &str) -> HarnessResult<UserCredentialStatus> {
    match raw {
        "pending" => Ok(UserCredentialStatus::Pending),
        "active" => Ok(UserCredentialStatus::Active),
        "invalid" => Ok(UserCredentialStatus::Invalid),
        _ => Err(HarnessError::Transport(format!(
            "unknown credential status from cli output: {raw}"
        ))),
    }
}

fn setting_key_name(key: UserSettingKey) -> &'static str {
    match key {
        UserSettingKey::Theme => "theme",
        UserSettingKey::Editor => "editor",
    }
}

fn setting_value_cli_arg(value: &UserSettingValue) -> String {
    match value {
        UserSettingValue::Theme(pref) => match pref {
            ThemePreference::System => "system".to_owned(),
            ThemePreference::Light => "light".to_owned(),
            ThemePreference::Dark => "dark".to_owned(),
        },
        UserSettingValue::Editor(editor) => editor.clone(),
    }
}

fn credential_kind_name(kind: UserCredentialKind) -> &'static str {
    match kind {
        UserCredentialKind::ProviderApiToken => "provider-api-token",
        UserCredentialKind::HarnessApiToken => "harness-api-token",
    }
}
