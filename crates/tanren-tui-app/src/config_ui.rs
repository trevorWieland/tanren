use secrecy::SecretString;
use tanren_app_services::AppServiceError;
use tanren_configuration_secrets::{
    CredentialId, CredentialKind, UserSettingKey, UserSettingValue,
};
use tanren_contract::{
    CreateCredentialRequest, CreateCredentialResponse, ListCredentialsResponse,
    ListUserConfigResponse, RemoveCredentialRequest, RemoveCredentialResponse,
    RemoveUserConfigRequest, RemoveUserConfigResponse, SetUserConfigRequest, SetUserConfigResponse,
    UpdateCredentialRequest, UpdateCredentialResponse,
};
use uuid::Uuid;

use crate::{FormField, FormState, OutcomeView};

pub(crate) fn user_config_set_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Key (preferred_harness / preferred_provider)",
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

pub(crate) fn user_config_remove_fields() -> Vec<FormField> {
    vec![FormField {
        label: "Key (preferred_harness / preferred_provider)",
        secret: false,
        value: String::new(),
    }]
}

pub(crate) fn credential_add_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Kind (api_key / source_control_token / webhook_signing_key / oidc_client_secret / opaque_secret)",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Name",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Description (optional)",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Provider (optional)",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Secret value",
            secret: true,
            value: String::new(),
        },
    ]
}

pub(crate) fn credential_update_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Credential ID (UUID)",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Name (optional)",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Description (optional)",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "New secret value",
            secret: true,
            value: String::new(),
        },
    ]
}

pub(crate) fn credential_remove_fields() -> Vec<FormField> {
    vec![FormField {
        label: "Credential ID (UUID)",
        secret: false,
        value: String::new(),
    }]
}

fn parse_setting_key(raw: &str) -> Result<UserSettingKey, String> {
    let trimmed = raw.trim();
    UserSettingKey::all()
        .iter()
        .find(|k| k.to_string() == trimmed)
        .copied()
        .ok_or_else(|| {
            format!(
                "validation_failed: unknown key '{trimmed}'. Valid: {}",
                UserSettingKey::all()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
}

fn parse_credential_kind(raw: &str) -> Result<CredentialKind, String> {
    let trimmed = raw.trim();
    match trimmed {
        "api_key" => Ok(CredentialKind::ApiKey),
        "source_control_token" => Ok(CredentialKind::SourceControlToken),
        "webhook_signing_key" => Ok(CredentialKind::WebhookSigningKey),
        "oidc_client_secret" => Ok(CredentialKind::OidcClientSecret),
        "opaque_secret" => Ok(CredentialKind::OpaqueSecret),
        _ => Err(format!(
            "validation_failed: unknown kind '{trimmed}'. Valid: api_key, source_control_token, webhook_signing_key, oidc_client_secret, opaque_secret"
        )),
    }
}

fn parse_credential_id(raw: &str) -> Result<CredentialId, String> {
    let trimmed = raw.trim();
    Uuid::parse_str(trimmed)
        .map(CredentialId::new)
        .map_err(|_| format!("validation_failed: invalid UUID: '{trimmed}'"))
}

fn optional_string(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

pub(crate) fn parse_user_config_set(state: &FormState) -> Result<SetUserConfigRequest, String> {
    let key = parse_setting_key(state.value(0))?;
    let value =
        UserSettingValue::parse(state.value(1)).map_err(|e| format!("validation_failed: {e}"))?;
    Ok(SetUserConfigRequest { key, value })
}

pub(crate) fn parse_user_config_remove(
    state: &FormState,
) -> Result<RemoveUserConfigRequest, String> {
    let key = parse_setting_key(state.value(0))?;
    Ok(RemoveUserConfigRequest { key })
}

pub(crate) fn parse_credential_add(state: &FormState) -> Result<CreateCredentialRequest, String> {
    let kind = parse_credential_kind(state.value(0))?;
    let name = state.value(1).trim().to_owned();
    if name.is_empty() {
        return Err("validation_failed: name is required".to_owned());
    }
    let description = optional_string(state.value(2));
    let provider = optional_string(state.value(3));
    let value = SecretString::from(state.value(4).to_owned());
    Ok(CreateCredentialRequest {
        kind,
        name,
        description,
        provider,
        value,
    })
}

pub(crate) fn parse_credential_update(
    state: &FormState,
) -> Result<UpdateCredentialRequest, String> {
    let id = parse_credential_id(state.value(0))?;
    let name = optional_string(state.value(1));
    let description = optional_string(state.value(2));
    let value = SecretString::from(state.value(3).to_owned());
    Ok(UpdateCredentialRequest {
        id,
        name,
        description,
        value,
    })
}

pub(crate) fn parse_credential_remove(
    state: &FormState,
) -> Result<RemoveCredentialRequest, String> {
    let id = parse_credential_id(state.value(0))?;
    Ok(RemoveCredentialRequest { id })
}

pub(crate) fn list_user_config_outcome(response: &ListUserConfigResponse) -> OutcomeView {
    let mut lines = vec!["User configuration:".to_owned()];
    if response.entries.is_empty() {
        lines.push("  (no settings configured)".to_owned());
    }
    for entry in &response.entries {
        lines.push(format!("  {} = {}", entry.key, entry.value));
    }
    OutcomeView {
        title: "User Configuration",
        lines,
    }
}

pub(crate) fn set_user_config_outcome(response: &SetUserConfigResponse) -> OutcomeView {
    OutcomeView {
        title: "Config Set",
        lines: vec![
            format!("key: {}", response.entry.key),
            format!("value: {}", response.entry.value),
        ],
    }
}

pub(crate) fn remove_user_config_outcome(response: &RemoveUserConfigResponse) -> OutcomeView {
    OutcomeView {
        title: "Config Removed",
        lines: vec![if response.removed {
            "setting removed".to_owned()
        } else {
            "no such setting".to_owned()
        }],
    }
}

pub(crate) fn list_credentials_outcome(response: &ListCredentialsResponse) -> OutcomeView {
    let mut lines = vec!["Credentials (metadata only, values never shown):".to_owned()];
    if response.credentials.is_empty() {
        lines.push("  (no credentials stored)".to_owned());
    }
    for cred in &response.credentials {
        lines.push(format!("  [{}] {} ({})", cred.id, cred.name, cred.kind));
        if let Some(desc) = &cred.description {
            lines.push(format!("    description: {desc}"));
        }
        lines.push(format!(
            "    scope: {}, present: {}",
            cred.scope, cred.present
        ));
    }
    OutcomeView {
        title: "Credentials",
        lines,
    }
}

pub(crate) fn create_credential_outcome(response: &CreateCredentialResponse) -> OutcomeView {
    let c = &response.credential;
    OutcomeView {
        title: "Credential Added",
        lines: vec![
            format!("id: {}", c.id),
            format!("name: {}", c.name),
            format!("kind: {}", c.kind),
        ],
    }
}

pub(crate) fn update_credential_outcome(response: &UpdateCredentialResponse) -> OutcomeView {
    let c = &response.credential;
    OutcomeView {
        title: "Credential Updated",
        lines: vec![
            format!("id: {}", c.id),
            format!("name: {}", c.name),
            format!("kind: {}", c.kind),
        ],
    }
}

pub(crate) fn remove_credential_outcome(response: &RemoveCredentialResponse) -> OutcomeView {
    OutcomeView {
        title: "Credential Removed",
        lines: vec![if response.removed {
            "credential removed".to_owned()
        } else {
            "no such credential".to_owned()
        }],
    }
}

pub(crate) fn render_config_error(err: AppServiceError) -> String {
    match err {
        AppServiceError::Configuration(reason) => {
            format!("{}: {}", reason.code(), reason.summary())
        }
        AppServiceError::InvalidInput(msg) => format!("validation_failed: {msg}"),
        AppServiceError::Store(e) => format!("internal_error: {e}"),
        _ => "internal_error: unknown failure".to_owned(),
    }
}
