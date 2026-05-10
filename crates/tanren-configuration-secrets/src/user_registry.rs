use crate::{
    ConfigurationValidationFailure, USER_SETTING_EDITOR_MAX_BYTES, UserCredentialKind,
    UserSettingKey, UserSettingValue, UserSettingValueKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserSettingDescriptor {
    pub key: UserSettingKey,
    pub wire_name: &'static str,
    pub value_kind: UserSettingValueKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserCredentialKindDescriptor {
    pub kind: UserCredentialKind,
    pub wire_name: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct UserSettingRegistryEntry {
    descriptor: UserSettingDescriptor,
    validator: fn(&UserSettingValue) -> Result<(), ConfigurationValidationFailure>,
}

#[derive(Debug, Clone, Copy)]
struct UserCredentialKindRegistryEntry {
    descriptor: UserCredentialKindDescriptor,
}

pub const USER_SETTING_DESCRIPTORS: [UserSettingDescriptor; 2] = [
    UserSettingDescriptor {
        key: UserSettingKey::Theme,
        wire_name: "theme",
        value_kind: UserSettingValueKind::Theme,
    },
    UserSettingDescriptor {
        key: UserSettingKey::Editor,
        wire_name: "editor",
        value_kind: UserSettingValueKind::Editor,
    },
];

pub const USER_CREDENTIAL_KIND_DESCRIPTORS: [UserCredentialKindDescriptor; 2] = [
    UserCredentialKindDescriptor {
        kind: UserCredentialKind::ProviderApiToken,
        wire_name: "provider_api_token",
    },
    UserCredentialKindDescriptor {
        kind: UserCredentialKind::HarnessApiToken,
        wire_name: "harness_api_token",
    },
];

const USER_SETTING_REGISTRY: [UserSettingRegistryEntry; USER_SETTING_DESCRIPTORS.len()] = [
    UserSettingRegistryEntry {
        descriptor: USER_SETTING_DESCRIPTORS[0],
        validator: validate_theme_setting_value,
    },
    UserSettingRegistryEntry {
        descriptor: USER_SETTING_DESCRIPTORS[1],
        validator: validate_editor_setting_value_variant,
    },
];

const USER_CREDENTIAL_KIND_REGISTRY: [UserCredentialKindRegistryEntry;
    USER_CREDENTIAL_KIND_DESCRIPTORS.len()] = [
    UserCredentialKindRegistryEntry {
        descriptor: USER_CREDENTIAL_KIND_DESCRIPTORS[0],
    },
    UserCredentialKindRegistryEntry {
        descriptor: USER_CREDENTIAL_KIND_DESCRIPTORS[1],
    },
];

/// Validate a user-tier setting payload.
///
/// # Errors
///
/// Returns [`ConfigurationValidationFailure`] if the value kind is wrong for
/// the key or an editor value is blank.
pub fn validate_user_setting(
    key: UserSettingKey,
    value: &UserSettingValue,
) -> Result<(), ConfigurationValidationFailure> {
    let Some(entry) = user_setting_registry_entry_for_key(key) else {
        return Err(ConfigurationValidationFailure::UnsupportedSettingKey {
            key: format!("{key:?}"),
        });
    };
    let provided = value.kind();
    if entry.descriptor.value_kind != provided {
        return Err(ConfigurationValidationFailure::SettingTypeMismatch {
            key,
            expected: entry.descriptor.value_kind,
            provided,
        });
    }

    (entry.validator)(value)
}

/// Validate that a credential kind is currently supported.
///
/// # Errors
///
/// Returns [`ConfigurationValidationFailure::UnsupportedCredentialKind`] when
/// the kind is not currently registered.
pub fn validate_user_credential_kind(
    kind: UserCredentialKind,
) -> Result<(), ConfigurationValidationFailure> {
    if user_credential_kind_registry_entry_for_kind(kind).is_some() {
        return Ok(());
    }
    Err(ConfigurationValidationFailure::UnsupportedCredentialKind {
        kind: format!("{kind:?}"),
    })
}

/// Parse a user-setting key through the supported settings registry.
///
/// # Errors
///
/// Returns [`ConfigurationValidationFailure::UnsupportedSettingKey`] when
/// `raw` does not map to a supported key.
pub fn parse_user_setting_key(raw: &str) -> Result<UserSettingKey, ConfigurationValidationFailure> {
    let trimmed = raw.trim();
    user_setting_registry_entry_for_wire_name(trimmed)
        .map(|entry| entry.descriptor.key)
        .ok_or_else(|| ConfigurationValidationFailure::UnsupportedSettingKey {
            key: trimmed.to_owned(),
        })
}

/// Resolve a user-setting key to its stable wire name through the registry.
///
/// # Errors
///
/// Returns [`ConfigurationValidationFailure::UnsupportedSettingKey`] when the
/// key is not currently registered.
pub fn user_setting_key_wire_name(
    key: UserSettingKey,
) -> Result<&'static str, ConfigurationValidationFailure> {
    user_setting_registry_entry_for_key(key)
        .map(|entry| entry.descriptor.wire_name)
        .ok_or_else(|| ConfigurationValidationFailure::UnsupportedSettingKey {
            key: format!("{key:?}"),
        })
}

/// Parse a user-credential kind through the supported kind registry.
///
/// # Errors
///
/// Returns [`ConfigurationValidationFailure::UnsupportedCredentialKind`] when
/// `raw` does not map to a supported kind.
pub fn parse_user_credential_kind(
    raw: &str,
) -> Result<UserCredentialKind, ConfigurationValidationFailure> {
    let trimmed = raw.trim();
    user_credential_kind_registry_entry_for_wire_name(trimmed)
        .map(|entry| entry.descriptor.kind)
        .ok_or_else(
            || ConfigurationValidationFailure::UnsupportedCredentialKind {
                kind: trimmed.to_owned(),
            },
        )
}

/// Resolve a user-credential kind to its stable wire name through the
/// registry.
///
/// # Errors
///
/// Returns [`ConfigurationValidationFailure::UnsupportedCredentialKind`] when
/// the kind is not currently registered.
pub fn user_credential_kind_wire_name(
    kind: UserCredentialKind,
) -> Result<&'static str, ConfigurationValidationFailure> {
    user_credential_kind_registry_entry_for_kind(kind)
        .map(|entry| entry.descriptor.wire_name)
        .ok_or_else(
            || ConfigurationValidationFailure::UnsupportedCredentialKind {
                kind: format!("{kind:?}"),
            },
        )
}

fn user_setting_registry_entry_for_key(
    key: UserSettingKey,
) -> Option<&'static UserSettingRegistryEntry> {
    USER_SETTING_REGISTRY
        .iter()
        .find(|entry| entry.descriptor.key == key)
}

fn user_setting_registry_entry_for_wire_name(
    wire_name: &str,
) -> Option<&'static UserSettingRegistryEntry> {
    USER_SETTING_REGISTRY
        .iter()
        .find(|entry| entry.descriptor.wire_name == wire_name)
}

fn user_credential_kind_registry_entry_for_kind(
    kind: UserCredentialKind,
) -> Option<&'static UserCredentialKindRegistryEntry> {
    USER_CREDENTIAL_KIND_REGISTRY
        .iter()
        .find(|entry| entry.descriptor.kind == kind)
}

fn user_credential_kind_registry_entry_for_wire_name(
    wire_name: &str,
) -> Option<&'static UserCredentialKindRegistryEntry> {
    USER_CREDENTIAL_KIND_REGISTRY
        .iter()
        .find(|entry| entry.descriptor.wire_name == wire_name)
}

fn validate_theme_setting_value(
    value: &UserSettingValue,
) -> Result<(), ConfigurationValidationFailure> {
    match value {
        UserSettingValue::Theme(_) => Ok(()),
        UserSettingValue::Editor(_) => Err(ConfigurationValidationFailure::SettingTypeMismatch {
            key: UserSettingKey::Theme,
            expected: UserSettingValueKind::Theme,
            provided: value.kind(),
        }),
    }
}

/// Validate a raw editor setting value string.
///
/// # Errors
///
/// Returns [`ConfigurationValidationFailure::EditorEmpty`] when the value
/// is blank after trimming, or [`ConfigurationValidationFailure::EditorTooLong`]
/// when the value exceeds [`USER_SETTING_EDITOR_MAX_BYTES`].
pub fn validate_editor_setting_value(raw: &str) -> Result<(), ConfigurationValidationFailure> {
    if raw.trim().is_empty() {
        return Err(ConfigurationValidationFailure::EditorEmpty);
    }
    let actual_bytes = raw.len();
    if actual_bytes > USER_SETTING_EDITOR_MAX_BYTES {
        return Err(ConfigurationValidationFailure::EditorTooLong {
            max_bytes: USER_SETTING_EDITOR_MAX_BYTES,
            actual_bytes,
        });
    }
    Ok(())
}

fn validate_editor_setting_value_variant(
    value: &UserSettingValue,
) -> Result<(), ConfigurationValidationFailure> {
    let UserSettingValue::Editor(editor) = value else {
        return Err(ConfigurationValidationFailure::SettingTypeMismatch {
            key: UserSettingKey::Editor,
            expected: UserSettingValueKind::Editor,
            provided: value.kind(),
        });
    };
    validate_editor_setting_value(editor.value())
}
