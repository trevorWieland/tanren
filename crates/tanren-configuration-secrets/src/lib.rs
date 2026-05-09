//! Configuration and Secrets subsystem.
//!
//! Configuration is tier-scoped (user, account, project, organization) with
//! deterministic inheritance. Secret values are encrypted at rest and never
//! recorded in event payloads, projection files, or proof artifacts; only
//! non-secret metadata is event-replayable.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tanren_identity_policy::AccountId;
use thiserror::Error;
use utoipa::ToSchema;

/// Configuration tiers in inheritance order, from most-specific to most-general.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema, ToSchema,
)]
#[non_exhaustive]
pub enum Tier {
    /// User-tier configuration. Most specific.
    User,
    /// Project-tier configuration.
    Project,
    /// Account-tier configuration.
    Account,
    /// Organization-tier configuration. Most general.
    Organization,
}

/// Closed set of user-tier setting keys covered by B-0048's initial contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UserSettingKey {
    /// Personal theme preference.
    Theme,
    /// Preferred editor command for user-initiated work.
    Editor,
}

/// Theme preference for user-tier configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    /// Follow the active client/system theme.
    System,
    /// Force light theme.
    Light,
    /// Force dark theme.
    Dark,
}

/// Type discriminator for user setting values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UserSettingValueKind {
    /// Theme preference value.
    Theme,
    /// Preferred editor value.
    Editor,
}

/// Typed value payload for a user-tier setting.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum UserSettingValue {
    /// Theme preference value.
    Theme(ThemePreference),
    /// Editor command value (`code`, `vim`, `nvim`, ...).
    Editor(String),
}

impl UserSettingKey {
    /// Value kind this key accepts.
    #[must_use]
    pub const fn expected_kind(self) -> UserSettingValueKind {
        match self {
            Self::Theme => UserSettingValueKind::Theme,
            Self::Editor => UserSettingValueKind::Editor,
        }
    }
}

impl UserSettingValue {
    /// Discriminator for this value payload.
    #[must_use]
    pub const fn kind(&self) -> UserSettingValueKind {
        match self {
            Self::Theme(_) => UserSettingValueKind::Theme,
            Self::Editor(_) => UserSettingValueKind::Editor,
        }
    }
}

/// Account-scoped owner shape for user-owned configuration and secrets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum OwnerScope {
    /// Data owned by exactly one signed-in account.
    User {
        /// Owning account id.
        account_id: AccountId,
    },
}

/// User-owned credential kinds covered by B-0048/B-0125's initial contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UserCredentialKind {
    /// Provider API token (e.g. model-provider access).
    ProviderApiToken,
    /// Harness API token for tool execution.
    HarnessApiToken,
}

/// Lifecycle status for stored user-owned credentials.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UserCredentialStatus {
    /// Metadata exists but the value has not yet been validated.
    Pending,
    /// Value is present and currently usable.
    Active,
    /// Value is present but failed recent validation.
    Invalid,
}

/// Replayable metadata view for a stored user-owned credential.
///
/// The raw value is never present here.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct UserCredentialMetadata {
    /// Stable credential identifier (slug), not the secret value.
    pub id: String,
    /// Declared credential kind.
    pub kind: UserCredentialKind,
    /// Owning scope for this credential.
    pub owner_scope: OwnerScope,
    /// Current lifecycle status.
    pub status: UserCredentialStatus,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Holder for a create/update write that carries a fresh secret value.
#[derive(Debug, Clone)]
pub struct UserCredentialWrite {
    /// Declared credential kind.
    pub kind: UserCredentialKind,
    /// Owning scope for this write.
    pub owner_scope: OwnerScope,
    /// Secret value; zeroized on drop.
    pub value: SecretString,
}

impl UserCredentialWrite {
    /// Validate the write payload before persistence.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigurationValidationFailure::ValueEmpty`] when the
    /// secret is blank/whitespace.
    pub fn validate(&self) -> Result<(), ConfigurationValidationFailure> {
        if self.value.expose_secret().trim().is_empty() {
            return Err(ConfigurationValidationFailure::ValueEmpty);
        }
        Ok(())
    }
}

/// Replayable metadata for a stored secret. The secret value itself is held
/// out-of-band by an encrypted store and never appears in this record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretMetadata {
    /// Stable secret identifier (slug, not value).
    pub id: String,
    /// Owning configuration tier.
    pub tier: Tier,
    /// Provider or harness this secret is associated with, if any.
    pub provider: Option<String>,
    /// True once a value has been written; false until the first set.
    pub present: bool,
}

/// Holder for a freshly-resolved secret value. The wrapper zeroes on drop.
#[derive(Debug, Clone)]
pub struct ResolvedSecret {
    /// Identifier this value resolved against.
    pub id: String,
    /// The secret value. Zeroed on drop via [`secrecy::SecretString`].
    pub value: SecretString,
}

/// Typed validation failures for user-tier setting and credential payloads.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(tag = "reason", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ConfigurationValidationFailure {
    /// Setting key does not match the supplied value kind.
    #[error("setting '{key:?}' requires value kind '{expected:?}', got '{provided:?}'")]
    SettingTypeMismatch {
        /// Key being written.
        key: UserSettingKey,
        /// Value kind expected by the key.
        expected: UserSettingValueKind,
        /// Value kind supplied by the caller.
        provided: UserSettingValueKind,
    },
    /// Editor command was blank after trimming.
    #[error("editor value is empty")]
    EditorEmpty,
    /// Secret write value was blank after trimming.
    #[error("secret value is empty")]
    ValueEmpty,
}

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
    let expected = key.expected_kind();
    let provided = value.kind();
    if expected != provided {
        return Err(ConfigurationValidationFailure::SettingTypeMismatch {
            key,
            expected,
            provided,
        });
    }

    if let UserSettingValue::Editor(editor) = value
        && editor.trim().is_empty()
    {
        return Err(ConfigurationValidationFailure::EditorEmpty);
    }

    Ok(())
}

/// Errors raised by configuration and secrets operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConfigSecretsError {
    /// Lookup found no value at any tier.
    #[error("no value found for key '{0}'")]
    NotFound(String),
    /// Input payload failed contract/domain validation.
    #[error("invalid input: {0}")]
    Validation(#[from] ConfigurationValidationFailure),
}
