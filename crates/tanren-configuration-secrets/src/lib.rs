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
use uuid::Uuid;

mod secret_store;
mod user_registry;
pub use secret_store::{
    CREDENTIAL_SEAL_PASSPHRASE_ENV, CredentialSealScheme, CredentialSealVersion,
    CredentialSealingFailure, CredentialValueSealer, SealedUserCredentialValue,
    UserCredentialSealContext, seal_user_credential_value_from_env,
};
pub use user_registry::{
    USER_CREDENTIAL_KIND_DESCRIPTORS, USER_SETTING_DESCRIPTORS, UserCredentialKindDescriptor,
    UserSettingDescriptor, parse_user_credential_kind, parse_user_setting_key,
    user_credential_kind_wire_name, user_setting_key_wire_name, validate_user_credential_kind,
    validate_user_setting,
};

/// Maximum byte length allowed for the editor setting value.
pub const USER_SETTING_EDITOR_MAX_BYTES: usize = 1_024;
/// Maximum byte length allowed for user credential secret values.
pub const USER_CREDENTIAL_SECRET_MAX_BYTES: usize = 8_192;
/// Minimum byte length required for credential-seal operator passphrases.
pub const CREDENTIAL_SEAL_PASSPHRASE_MIN_BYTES: usize = 24;
/// Minimum estimated entropy bits required for credential-seal operator passphrases.
pub const CREDENTIAL_SEAL_PASSPHRASE_MIN_ESTIMATED_ENTROPY_BITS: usize = 72;

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

/// Stable identifier for user-owned credential metadata rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String, format = "uuid")]
pub struct UserCredentialId(Uuid);

impl UserCredentialId {
    /// Wrap a raw UUID.
    #[must_use]
    pub const fn new(value: Uuid) -> Self {
        Self(value)
    }

    /// Allocate a fresh stable id.
    #[must_use]
    pub fn fresh() -> Self {
        Self(Uuid::now_v7())
    }

    /// Parse a user credential id from a raw string.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigurationValidationFailure::CredentialIdInvalid`] when
    /// the value is not a valid UUID.
    pub fn parse(raw: &str) -> Result<Self, ConfigurationValidationFailure> {
        let parsed = Uuid::parse_str(raw).map_err(|_| {
            ConfigurationValidationFailure::CredentialIdInvalid {
                value: raw.to_owned(),
            }
        })?;
        Ok(Self::new(parsed))
    }

    /// The wrapped UUID value.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl std::fmt::Display for UserCredentialId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
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
    pub id: UserCredentialId,
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

/// Validated passphrase used to derive per-credential sealing keys.
///
/// Validation enforces minimum size and a minimum estimated entropy budget.
#[derive(Clone)]
pub struct CredentialSealPassphrase(SecretString);

impl std::fmt::Debug for CredentialSealPassphrase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CredentialSealPassphrase(<redacted>)")
    }
}

impl CredentialSealPassphrase {
    /// Parse and validate a credential-seal passphrase.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialSealPassphraseValidationFailure`] when the
    /// passphrase does not meet minimum size or entropy checks.
    pub fn parse(value: &str) -> Result<Self, CredentialSealPassphraseValidationFailure> {
        validate_credential_seal_passphrase(value)?;
        Ok(Self(SecretString::new(value.to_owned().into_boxed_str())))
    }

    /// Access the passphrase bytes for key derivation.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.expose_secret().as_bytes()
    }
}

impl UserCredentialWrite {
    /// Validate the write payload before persistence.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigurationValidationFailure`] when the secret is
    /// blank/whitespace or exceeds the maximum supported byte length.
    pub fn validate(&self) -> Result<(), ConfigurationValidationFailure> {
        validate_user_credential_value(&self.value)
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
    /// Editor command exceeds the supported byte length bound.
    #[error("editor value exceeds max byte length ({actual_bytes}>{max_bytes})")]
    EditorTooLong {
        /// Maximum byte length accepted by the contract.
        max_bytes: usize,
        /// Provided byte length.
        actual_bytes: usize,
    },
    /// Secret write value was blank after trimming.
    #[error("secret value is empty")]
    ValueEmpty,
    /// Secret write value exceeds the supported byte length bound.
    #[error("secret value exceeds max byte length ({actual_bytes}>{max_bytes})")]
    ValueTooLong {
        /// Maximum byte length accepted by the contract.
        max_bytes: usize,
        /// Provided byte length.
        actual_bytes: usize,
    },
    /// Raw user setting key is not part of the current supported registry.
    #[error("unsupported user setting key '{key}'")]
    UnsupportedSettingKey {
        /// Unsupported raw setting key from the caller.
        key: String,
    },
    /// Raw user credential kind is not part of the current supported registry.
    #[error("unsupported user credential kind '{kind}'")]
    UnsupportedCredentialKind {
        /// Unsupported raw credential kind from the caller.
        kind: String,
    },
    /// Credential metadata id is not a valid UUID.
    #[error("credential id is not a valid uuid: '{value}'")]
    CredentialIdInvalid {
        /// Invalid raw credential id.
        value: String,
    },
}

/// Validation failures for credential-seal operator passphrases.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum CredentialSealPassphraseValidationFailure {
    /// Passphrase is shorter than the minimum supported byte length.
    #[error(
        "credential seal operator passphrase is too short ({actual_bytes} bytes); minimum is {min_bytes} bytes"
    )]
    TooShort {
        /// Required minimum byte length.
        min_bytes: usize,
        /// Provided byte length.
        actual_bytes: usize,
    },
    /// Passphrase failed the minimum entropy heuristic.
    #[error(
        "credential seal operator passphrase entropy is too low ({estimated_bits} bits); minimum is {min_estimated_bits} bits"
    )]
    LowEntropy {
        /// Required minimum estimated entropy bits.
        min_estimated_bits: usize,
        /// Estimated entropy bits from the heuristic.
        estimated_bits: usize,
    },
}

/// Validate a user-owned credential secret before persistence/encryption work.
///
/// # Errors
///
/// Returns [`ConfigurationValidationFailure`] if the secret is blank/whitespace
/// or exceeds the configured byte-length bound.
pub fn validate_user_credential_value(
    value: &SecretString,
) -> Result<(), ConfigurationValidationFailure> {
    if value.expose_secret().trim().is_empty() {
        return Err(ConfigurationValidationFailure::ValueEmpty);
    }
    let actual_bytes = value.expose_secret().len();
    if actual_bytes > USER_CREDENTIAL_SECRET_MAX_BYTES {
        return Err(ConfigurationValidationFailure::ValueTooLong {
            max_bytes: USER_CREDENTIAL_SECRET_MAX_BYTES,
            actual_bytes,
        });
    }
    Ok(())
}

/// Validate a credential-seal passphrase used for at-rest credential encryption.
///
/// # Errors
///
/// Returns [`CredentialSealPassphraseValidationFailure`] if the passphrase is
/// shorter than the minimum supported byte length or fails the minimum entropy
/// heuristic.
pub fn validate_credential_seal_passphrase(
    value: &str,
) -> Result<(), CredentialSealPassphraseValidationFailure> {
    let bytes = value.as_bytes();
    let actual_bytes = bytes.len();
    if actual_bytes < CREDENTIAL_SEAL_PASSPHRASE_MIN_BYTES {
        return Err(CredentialSealPassphraseValidationFailure::TooShort {
            min_bytes: CREDENTIAL_SEAL_PASSPHRASE_MIN_BYTES,
            actual_bytes,
        });
    }
    let estimated_bits = estimated_entropy_bits(bytes);
    if estimated_bits < CREDENTIAL_SEAL_PASSPHRASE_MIN_ESTIMATED_ENTROPY_BITS {
        return Err(CredentialSealPassphraseValidationFailure::LowEntropy {
            min_estimated_bits: CREDENTIAL_SEAL_PASSPHRASE_MIN_ESTIMATED_ENTROPY_BITS,
            estimated_bits,
        });
    }
    Ok(())
}

fn estimated_entropy_bits(bytes: &[u8]) -> usize {
    let mut seen = [false; 256];
    let mut unique_symbols = 0_usize;
    for byte in bytes {
        let index = usize::from(*byte);
        if !seen[index] {
            seen[index] = true;
            unique_symbols += 1;
        }
    }

    let bits_per_symbol = unique_symbols
        .checked_ilog2()
        .map_or(0_usize, |bits| bits as usize);
    bytes.len().saturating_mul(bits_per_symbol)
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
    /// Installation credential-seal configuration failed validation.
    #[error("invalid credential seal configuration: {0}")]
    CredentialSealConfiguration(#[from] CredentialSealPassphraseValidationFailure),
    /// Runtime credential sealing failed.
    #[error(transparent)]
    CredentialSealing(#[from] CredentialSealingFailure),
}
