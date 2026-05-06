//! Configuration and Secrets subsystem.
//!
//! Configuration is tier-scoped (user, account, project, organization) with
//! deterministic inheritance. Secret values are encrypted at rest and never
//! recorded in event payloads, projection files, or proof artifacts; only
//! non-secret metadata is event-replayable.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use utoipa::ToSchema;
use uuid::Uuid;

/// Maximum accepted byte length for a user-setting value.
const USER_SETTING_MAX_LEN: usize = 128;

/// Configuration tiers in inheritance order, from most-specific to most-general.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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

/// Closed set of user-tier configuration setting keys proven by R-0008.
///
/// Each variant maps to one setting whose value is validated through
/// [`UserSettingValue::parse`]. Notification preferences are owned by
/// R-0010 and intentionally absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum UserSettingKey {
    /// Preferred execution harness name (e.g. `"claude"`, `"codex"`).
    PreferredHarness,
    /// Preferred provider integration name (e.g. `"openai"`, `"anthropic"`).
    PreferredProvider,
}

impl UserSettingKey {
    /// All known user-tier setting keys.
    pub fn all() -> &'static [Self] {
        &[Self::PreferredHarness, Self::PreferredProvider]
    }
}

/// Validated user-tier setting value.
///
/// Constructed through [`UserSettingValue::parse`] which trims surrounding
/// whitespace, rejects empty / whitespace-only input, rejects values
/// exceeding [`USER_SETTING_MAX_LEN`] bytes, and rejects values containing
/// control characters. The inner string is the trimmed, validated form.
///
/// Does NOT derive `Deserialize` directly — the custom impl below routes
/// every wire input through `parse`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
pub struct UserSettingValue(String);

impl UserSettingValue {
    /// Parse a raw user-supplied setting value.
    ///
    /// # Errors
    ///
    /// Returns [`SettingValidationError`] when the input is empty after
    /// trimming, exceeds the maximum length, or contains control
    /// characters.
    pub fn parse(raw: &str) -> Result<Self, SettingValidationError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(SettingValidationError::Empty);
        }
        if trimmed.len() > USER_SETTING_MAX_LEN {
            return Err(SettingValidationError::TooLong {
                max: USER_SETTING_MAX_LEN,
            });
        }
        if trimmed.chars().any(char::is_control) {
            return Err(SettingValidationError::ContainsControlCharacters);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Borrow the validated inner string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for UserSettingValue {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for UserSettingValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Stable identifier for a stored credential. `UUIDv7` — sortable + unique.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String, format = "uuid")]
pub struct CredentialId(Uuid);

impl CredentialId {
    /// Wrap a raw UUID.
    #[must_use]
    pub const fn new(value: Uuid) -> Self {
        Self(value)
    }

    /// Allocate a fresh time-ordered id.
    #[must_use]
    pub fn fresh() -> Self {
        Self(Uuid::now_v7())
    }

    /// The underlying UUID.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl From<Uuid> for CredentialId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl AsRef<Uuid> for CredentialId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl std::fmt::Display for CredentialId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Typed credential-kind registry.
///
/// Each variant declares a stable kind identifier. Core Tanren defines
/// common kinds; provider and secret-store adapters may register
/// additional kinds through the registry contract described in
/// `docs/architecture/subsystems/configuration-secrets.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CredentialKind {
    /// Generic API key (provider, service, ...).
    ApiKey,
    /// Source-control personal access token.
    SourceControlToken,
    /// Webhook signing secret.
    WebhookSigningKey,
    /// OIDC client secret.
    OidcClientSecret,
    /// Opaque secret with no structured interpretation.
    OpaqueSecret,
}

/// Ownership scope for a credential.
///
/// Each credential kind declares which scopes are valid. User-owned
/// credentials are the focus of R-0008; project and organization scopes
/// are owned by R-0011.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CredentialScope {
    /// Tied to one user login identity.
    User,
    /// Scoped to one project.
    Project,
    /// Shared at organization level.
    Organization,
    /// Belongs to a service account.
    ServiceAccount,
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

/// Redacted credential metadata view.
///
/// Carries only `kind`, `scope`, `updated_at`, and `present`. Identifying
/// fields such as `id`, `name`, `description`, `provider`, and `created_at`
/// are intentionally omitted so that credential management responses never
/// expose more than the minimal governance metadata required by R-0008.
/// The stored secret value is **never** included — it is write-only/use-only
/// after storage per core invariant 2.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct RedactedCredentialMetadata {
    /// Credential kind from the typed registry.
    pub kind: CredentialKind,
    /// Ownership scope.
    pub scope: CredentialScope,
    /// Wall-clock time the credential value was last replaced.
    pub updated_at: Option<DateTime<Utc>>,
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

/// Errors raised when a user-tier setting value fails validation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum SettingValidationError {
    /// The value was empty after trimming surrounding whitespace.
    #[error("setting value is empty")]
    Empty,
    /// The value exceeded the maximum accepted byte length.
    #[error("setting value exceeds {max} bytes")]
    TooLong { max: usize },
    /// The value contained one or more control characters.
    #[error("setting value contains control characters")]
    ContainsControlCharacters,
}

/// Errors raised by configuration and secrets operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConfigSecretsError {
    /// Lookup found no value at any tier.
    #[error("no value found for key '{0}'")]
    NotFound(String),
    /// A credential with the given id does not exist.
    #[error("credential not found: {0}")]
    CredentialNotFound(CredentialId),
    /// A credential with the same name and kind already exists for this
    /// owner.
    #[error("a credential named '{0}' of this kind already exists")]
    DuplicateCredentialName(String),
    /// Setting-value validation failed.
    #[error(transparent)]
    SettingValidation(#[from] SettingValidationError),
}
