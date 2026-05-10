//! Form factories, outcome adapters, and error-message helpers for
//! the TUI. Split out of `main.rs` to keep that file under the
//! workspace 500-line budget.
use crate::{FormField, FormState, OutcomeView};
use secrecy::SecretString;
use tanren_app_services::AppServiceError;
use tanren_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AccountFailureReason, SignInRequest,
    SignInResponse, SignUpRequest, SignUpResponse, ThemePreference, UserCredentialId,
    UserCredentialKind, UserCredentialStatus, UserCredentialView, UserSettingKey, UserSettingValue,
    parse_user_credential_kind, parse_user_setting_key,
};
use tanren_identity_policy::{AccountId, Email, InvitationToken, ValidationError};
use uuid::Uuid;
pub(crate) fn sign_up_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Email",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Password",
            secret: true,
            value: String::new(),
        },
        FormField {
            label: "Display name",
            secret: false,
            value: String::new(),
        },
    ]
}
pub(crate) fn sign_in_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Email",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Password",
            secret: true,
            value: String::new(),
        },
    ]
}
pub(crate) fn accept_invitation_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Invitation token",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Email",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Password",
            secret: true,
            value: String::new(),
        },
        FormField {
            label: "Display name",
            secret: false,
            value: String::new(),
        },
    ]
}
pub(crate) fn sign_up_outcome(response: &SignUpResponse) -> OutcomeView {
    OutcomeView {
        title: "Account created",
        lines: vec![
            format!("account_id: {}", response.account.id),
            format!("session token: {}", response.session.token.expose_secret()),
        ],
    }
}
pub(crate) fn sign_in_outcome(response: &SignInResponse) -> OutcomeView {
    OutcomeView {
        title: "Signed in",
        lines: vec![
            format!("account_id: {}", response.account.id),
            format!("session token: {}", response.session.token.expose_secret()),
        ],
    }
}
pub(crate) fn accept_invitation_outcome(response: &AcceptInvitationResponse) -> OutcomeView {
    OutcomeView {
        title: "Invitation accepted",
        lines: vec![
            format!("account_id: {}", response.account.id),
            format!("joined org: {}", response.joined_org),
            format!("session token: {}", response.session.token.expose_secret()),
        ],
    }
}
pub(crate) fn format_failure(reason: AccountFailureReason) -> String {
    format!("{}: {}", reason.code(), reason.summary())
}
pub(crate) fn render_error(err: AppServiceError) -> String {
    match err {
        AppServiceError::Account(reason) => format_failure(reason),
        AppServiceError::Configuration(reason) => {
            format!("{}: {}", reason.code(), reason.summary())
        }
        AppServiceError::InvalidInput(message) => format!("validation_failed: {message}"),
        AppServiceError::Store(_) => {
            "internal_error: Tanren encountered an internal error.".to_owned()
        }
        _ => "internal_error: unknown app-service failure".to_owned(),
    }
}
fn validation_message(err: &ValidationError) -> String {
    format!("validation_failed: {err}")
}
pub(crate) fn parse_sign_up(state: &FormState) -> Result<SignUpRequest, String> {
    let email = Email::parse(state.value(0)).map_err(|e| validation_message(&e))?;
    let password = SecretString::from(state.value(1).to_owned());
    let display_name = state.value(2).to_owned();
    Ok(SignUpRequest {
        email,
        password,
        display_name,
    })
}
pub(crate) fn config_list_settings_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Requested account id",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Limit (optional)",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "After cursor (optional)",
            secret: false,
            value: String::new(),
        },
    ]
}
pub(crate) fn config_set_setting_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Requested account id",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Key (theme|editor)",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Value",
            secret: false,
            value: String::new(),
        },
    ]
}
pub(crate) fn config_remove_setting_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Requested account id",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Key (theme|editor)",
            secret: false,
            value: String::new(),
        },
    ]
}
pub(crate) fn config_list_credentials_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Requested account id",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Limit (optional)",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "After cursor (optional)",
            secret: false,
            value: String::new(),
        },
    ]
}
pub(crate) fn config_add_credential_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Requested account id",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Kind (provider_api_token|harness_api_token)",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Value",
            secret: true,
            value: String::new(),
        },
    ]
}
pub(crate) fn config_update_credential_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Requested account id",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Item id",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Value",
            secret: true,
            value: String::new(),
        },
    ]
}
pub(crate) fn config_remove_credential_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Requested account id",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Item id",
            secret: false,
            value: String::new(),
        },
    ]
}
pub(crate) fn parse_account_id_field(state: &FormState) -> Result<AccountId, String> {
    parse_account_id(state.value(0))
}
pub(crate) fn parse_setting_key_field(
    state: &FormState,
    idx: usize,
) -> Result<UserSettingKey, String> {
    parse_user_setting_key(state.value(idx).trim())
        .map_err(|_| "validation_failed: key must be one of theme|editor".to_owned())
}
pub(crate) fn parse_setting_value_field(
    key: UserSettingKey,
    state: &FormState,
    idx: usize,
) -> Result<UserSettingValue, String> {
    match key {
        UserSettingKey::Theme => match state.value(idx).trim() {
            "system" => Ok(UserSettingValue::Theme(ThemePreference::System)),
            "light" => Ok(UserSettingValue::Theme(ThemePreference::Light)),
            "dark" => Ok(UserSettingValue::Theme(ThemePreference::Dark)),
            _ => Err("validation_failed: theme value must be system|light|dark".to_owned()),
        },
        UserSettingKey::Editor => Ok(UserSettingValue::Editor(state.value(idx).to_owned())),
    }
}
pub(crate) fn parse_credential_kind_field(
    state: &FormState,
    idx: usize,
) -> Result<UserCredentialKind, String> {
    parse_user_credential_kind(state.value(idx).trim()).map_err(|_| {
        "validation_failed: kind must be provider_api_token|harness_api_token".to_owned()
    })
}
pub(crate) fn credential_list_outcome(
    items: &[UserCredentialView],
    next_cursor: Option<&str>,
) -> OutcomeView {
    if items.is_empty() {
        let mut lines = vec!["No credentials found.".to_owned()];
        if let Some(cursor) = next_cursor {
            lines.push(format!("next_cursor={cursor}"));
        }
        return OutcomeView {
            title: "Credentials",
            lines,
        };
    }
    let mut lines = Vec::with_capacity(items.len() + usize::from(next_cursor.is_some()));
    for item in items {
        lines.push(format!(
            "{} kind={} status={} updated_at={}",
            item.id,
            credential_kind(item.kind),
            credential_status(item.status),
            item.updated_at.to_rfc3339()
        ));
    }
    if let Some(cursor) = next_cursor {
        lines.push(format!("next_cursor={cursor}"));
    }
    OutcomeView {
        title: "Credentials",
        lines,
    }
}
pub(crate) fn credential_item_outcome(
    title: &'static str,
    item: &UserCredentialView,
) -> OutcomeView {
    OutcomeView {
        title,
        lines: vec![
            format!("id: {}", item.id),
            format!("kind: {}", credential_kind(item.kind)),
            format!("status: {}", credential_status(item.status)),
            format!("updated_at: {}", item.updated_at.to_rfc3339()),
            "secret: [redacted]".to_owned(),
        ],
    }
}
pub(crate) fn settings_list_outcome(
    items: &[tanren_contract::UserSettingView],
    next_cursor: Option<&str>,
) -> OutcomeView {
    if items.is_empty() {
        let mut lines = vec!["No settings found.".to_owned()];
        if let Some(cursor) = next_cursor {
            lines.push(format!("next_cursor={cursor}"));
        }
        return OutcomeView {
            title: "User settings",
            lines,
        };
    }
    let mut lines = Vec::with_capacity(items.len() + usize::from(next_cursor.is_some()));
    for item in items {
        lines.push(format!(
            "{}={} updated_at={}",
            setting_key(item.key),
            setting_value(&item.value),
            item.updated_at.to_rfc3339()
        ));
    }
    if let Some(cursor) = next_cursor {
        lines.push(format!("next_cursor={cursor}"));
    }
    OutcomeView {
        title: "User settings",
        lines,
    }
}
pub(crate) fn setting_item_outcome(
    title: &'static str,
    item: &tanren_contract::UserSettingView,
) -> OutcomeView {
    OutcomeView {
        title,
        lines: vec![
            format!("key: {}", setting_key(item.key)),
            format!("value: {}", setting_value(&item.value)),
            format!("updated_at: {}", item.updated_at.to_rfc3339()),
        ],
    }
}
pub(crate) fn parse_item_id_field(
    state: &FormState,
    idx: usize,
) -> Result<UserCredentialId, String> {
    let raw = state.value(idx).trim();
    UserCredentialId::parse(raw)
        .map_err(|_| "validation_failed: item id must be a valid uuid".to_owned())
}
pub(crate) fn parse_list_limit_field(state: &FormState, idx: usize) -> Result<Option<u16>, String> {
    let raw = state.value(idx).trim();
    if raw.is_empty() {
        return Ok(None);
    }
    let parsed = raw
        .parse::<u16>()
        .map_err(|_| "validation_failed: limit must be a positive integer".to_owned())?;
    if parsed == 0 {
        return Err("validation_failed: limit must be a positive integer".to_owned());
    }
    Ok(Some(parsed))
}
pub(crate) fn parse_list_after_field(state: &FormState, idx: usize) -> Option<String> {
    let raw = state.value(idx).trim();
    if raw.is_empty() {
        return None;
    }
    Some(raw.to_owned())
}
fn parse_account_id(raw: &str) -> Result<AccountId, String> {
    let trimmed = raw.trim();
    let parsed = Uuid::parse_str(trimmed)
        .map_err(|_| "validation_failed: account id must be a uuid".to_owned())?;
    Ok(AccountId::new(parsed))
}
fn setting_key(key: UserSettingKey) -> &'static str {
    match key {
        UserSettingKey::Theme => "theme",
        UserSettingKey::Editor => "editor",
    }
}
fn setting_value(value: &UserSettingValue) -> String {
    match value {
        UserSettingValue::Theme(theme) => match theme {
            ThemePreference::System => "system".to_owned(),
            ThemePreference::Light => "light".to_owned(),
            ThemePreference::Dark => "dark".to_owned(),
        },
        UserSettingValue::Editor(editor) => editor.clone(),
    }
}
fn credential_kind(kind: UserCredentialKind) -> &'static str {
    match kind {
        UserCredentialKind::ProviderApiToken => "provider_api_token",
        UserCredentialKind::HarnessApiToken => "harness_api_token",
    }
}
fn credential_status(status: UserCredentialStatus) -> &'static str {
    match status {
        UserCredentialStatus::Pending => "pending",
        UserCredentialStatus::Active => "active",
        UserCredentialStatus::Invalid => "invalid",
    }
}
pub(crate) fn parse_sign_in(state: &FormState) -> Result<SignInRequest, String> {
    let email = Email::parse(state.value(0)).map_err(|e| validation_message(&e))?;
    let password = SecretString::from(state.value(1).to_owned());
    Ok(SignInRequest { email, password })
}
pub(crate) fn parse_accept_invitation(
    state: &FormState,
) -> Result<AcceptInvitationRequest, String> {
    let invitation_token =
        InvitationToken::parse(state.value(0)).map_err(|e| validation_message(&e))?;
    // The user supplies the email directly; the previous implementation
    // synthesised it from the invitation token, which broke any token
    // containing `@` (the resulting "<token>@invitation.tanren" had two
    // `@` characters and Email::parse rejected it before the request
    // ever reached `accept_invitation`). Codex P2 review on PR #133.
    let email = Email::parse(state.value(1)).map_err(|e| validation_message(&e))?;
    let password = SecretString::from(state.value(2).to_owned());
    let display_name = state.value(3).to_owned();
    Ok(AcceptInvitationRequest {
        invitation_token,
        email,
        password,
        display_name,
    })
}
