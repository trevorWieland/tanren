//! User-configuration and credential command/response wire shapes.
//!
//! These types are the request/response surface used by the api, mcp,
//! cli, tui, and web client when callers view, set, update, or remove
//! user-tier configuration values and when they add, update, or inspect
//! user-owned credentials. They live in `tanren-contract` because every
//! interface binary serialises the same shapes — keeping them here is the
//! architectural guarantee that the surfaces stay equivalent.
//!
//! # Security properties
//!
//! Secret-bearing request fields use [`SecretString`]; response and view
//! types never carry a raw secret-value field. Stored credential values
//! are write-only/use-only after submission (core invariant 2).

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tanren_configuration_secrets::{
    CredentialId, CredentialKind, CredentialScope, RedactedCredentialMetadata, UserSettingKey,
    UserSettingValue,
};
use tanren_identity_policy::AccountId;
use tanren_identity_policy::secret_serde;
use utoipa::ToSchema;

// ---------------------------------------------------------------------------
// User-tier configuration
// ---------------------------------------------------------------------------

/// Request to set (upsert) a single user-tier configuration value.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SetUserConfigRequest {
    /// Which user-tier setting to set.
    pub key: UserSettingKey,
    /// Validated setting value.
    pub value: UserSettingValue,
}

/// Successful set-user-config response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SetUserConfigResponse {
    /// The entry as persisted.
    pub entry: UserConfigEntry,
}

/// Request to retrieve a single user-tier configuration value.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct GetUserConfigRequest {
    /// Which user-tier setting to look up.
    pub key: UserSettingKey,
}

/// Successful get-user-config response. `entry` is `None` when the
/// requested key has no value set at the user tier.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct GetUserConfigResponse {
    /// The resolved entry, or `None` if the key has no user-tier value.
    pub entry: Option<UserConfigEntry>,
}

/// Request to remove a single user-tier configuration value.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct RemoveUserConfigRequest {
    /// Which user-tier setting to remove.
    pub key: UserSettingKey,
}

/// Successful remove-user-config response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct RemoveUserConfigResponse {
    /// `true` when a value was present and removed; `false` when the key
    /// was already absent.
    pub removed: bool,
}

/// Successful list-user-config response. Only keys that have a value set
/// at the user tier are included.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListUserConfigResponse {
    /// All user-tier entries that currently have a value.
    pub entries: Vec<UserConfigEntry>,
}

/// A single user-tier configuration key–value pair.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct UserConfigEntry {
    /// The setting key.
    pub key: UserSettingKey,
    /// The validated value.
    pub value: UserSettingValue,
    /// Wall-clock time at which this entry was last set.
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// User-owned credentials
// ---------------------------------------------------------------------------

/// Request to add a new user-owned credential.
///
/// The `value` field carries the secret through the wire once and is
/// never returned in any response or view type.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CreateCredentialRequest {
    /// Credential kind from the typed registry.
    pub kind: CredentialKind,
    /// Human-readable name unique per owner + kind.
    pub name: String,
    /// Optional longer description.
    pub description: Option<String>,
    /// Provider or adapter this credential targets, if applicable.
    pub provider: Option<String>,
    /// The secret value. Wrapped in [`SecretString`] so accidental
    /// `Debug` / `Serialize` calls do not leak the credential.
    #[serde(
        deserialize_with = "secret_serde::deserialize_password",
        serialize_with = "secret_serde::serialize_password_expose"
    )]
    #[schemars(with = "String")]
    #[schema(value_type = String)]
    pub value: SecretString,
}

/// Successful create-credential response. The stored value is never
/// included — only redacted metadata is returned.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CreateCredentialResponse {
    /// Redacted metadata for the newly stored credential.
    pub credential: RedactedCredentialMetadata,
}

/// Request to replace the value (and optionally metadata) of an existing
/// user-owned credential.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct UpdateCredentialRequest {
    /// Stable credential identifier to update.
    pub id: CredentialId,
    /// Updated human-readable name, if changing.
    pub name: Option<String>,
    /// Updated description, if changing.
    pub description: Option<String>,
    /// The replacement secret value. Wrapped in [`SecretString`] so
    /// accidental `Debug` / `Serialize` calls do not leak it.
    #[serde(
        deserialize_with = "secret_serde::deserialize_password",
        serialize_with = "secret_serde::serialize_password_expose"
    )]
    #[schemars(with = "String")]
    #[schema(value_type = String)]
    pub value: SecretString,
}

/// Successful update-credential response. The stored value is never
/// included — only redacted metadata is returned.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct UpdateCredentialResponse {
    /// Redacted metadata after the update.
    pub credential: RedactedCredentialMetadata,
}

/// Request to remove a user-owned credential.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct RemoveCredentialRequest {
    /// Stable credential identifier to remove.
    pub id: CredentialId,
}

/// Successful remove-credential response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct RemoveCredentialResponse {
    /// `true` when the credential existed and was removed.
    pub removed: bool,
}

/// Successful list-credentials response. Only redacted metadata is
/// returned — stored values are never projected.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListCredentialsResponse {
    /// Redacted metadata for each credential owned by the authenticated
    /// user.
    pub credentials: Vec<RedactedCredentialMetadata>,
}

/// Summary view of a credential's owner, kind, and scope. Used in
/// credential-usage tracking (R-0012) and audit projections.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CredentialOwnershipView {
    /// The user who owns this credential.
    pub owner: AccountId,
    /// Credential kind.
    pub kind: CredentialKind,
    /// Ownership scope.
    pub scope: CredentialScope,
}

// ---------------------------------------------------------------------------
// Failure taxonomy
// ---------------------------------------------------------------------------

/// Closed taxonomy of user-configuration and credential-flow failures.
///
/// Maps onto the shared `{code, summary}` error body documented in
/// `docs/architecture/subsystems/interfaces.md` "Error Taxonomy". Every
/// interface (api/mcp/cli/tui/web) projects a
/// `ConfigurationFailureReason` into the same wire shape so callers can
/// match on `code` regardless of transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ConfigurationFailureReason {
    /// The requested configuration key has no value at the user tier.
    SettingNotFound,
    /// The submitted setting value failed contract-level validation
    /// (empty, too long, control characters, ...). Distinct from
    /// `InvalidSettingKey` so callers can tell "bad key" from "bad value".
    InvalidSettingValue,
    /// The requested setting key is not a recognized user-tier key.
    InvalidSettingKey,
    /// The requested credential does not exist.
    CredentialNotFound,
    /// A credential with the same name and kind already exists for this
    /// owner.
    DuplicateCredentialName,
    /// The credential kind is not allowed at the requested scope.
    CredentialKindScopeMismatch,
    /// User-supplied input failed generic validation before any
    /// domain logic could run (malformed id, missing required fields,
    /// ...).
    ValidationFailed,
    /// The authenticated user is not authorized to perform the
    /// requested operation on this credential or configuration entry.
    Unauthorized,
    /// The requested notification channel is not supported by the
    /// current deployment or user's device configuration.
    UnsupportedNotificationChannel,
    /// The authenticated user is not authorized to set notification
    /// overrides for the target organization (e.g. non-admin role).
    UnauthorizedOrganizationOverride,
}

impl ConfigurationFailureReason {
    /// Stable wire `code` for this failure.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::SettingNotFound => "setting_not_found",
            Self::InvalidSettingValue => "invalid_setting_value",
            Self::InvalidSettingKey => "invalid_setting_key",
            Self::CredentialNotFound => "credential_not_found",
            Self::DuplicateCredentialName => "duplicate_credential_name",
            Self::CredentialKindScopeMismatch => "credential_kind_scope_mismatch",
            Self::ValidationFailed => "validation_failed",
            Self::Unauthorized => "unauthorized",
            Self::UnsupportedNotificationChannel => "unsupported_notification_channel",
            Self::UnauthorizedOrganizationOverride => "unauthorized_organization_override",
        }
    }

    /// Human-readable wire `summary` for this failure.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::SettingNotFound => "The requested configuration key has no user-tier value.",
            Self::InvalidSettingValue => {
                "The submitted setting value did not satisfy contract-level validation."
            }
            Self::InvalidSettingKey => {
                "The requested setting key is not a recognized user-tier key."
            }
            Self::CredentialNotFound => "No credential matches the supplied identifier.",
            Self::DuplicateCredentialName => {
                "A credential with this name and kind already exists for the owner."
            }
            Self::CredentialKindScopeMismatch => {
                "This credential kind is not valid at the requested scope."
            }
            Self::ValidationFailed => {
                "The submitted input did not satisfy contract-level validation."
            }
            Self::Unauthorized => "The authenticated user is not authorized for this operation.",
            Self::UnsupportedNotificationChannel => {
                "The requested notification channel is not supported."
            }
            Self::UnauthorizedOrganizationOverride => {
                "The authenticated user is not authorized to set notification overrides for this organization."
            }
        }
    }

    /// Recommended HTTP status for the failure when projected over the
    /// api / mcp surfaces. Centralized so every transport reports the
    /// same status for the same failure code.
    #[must_use]
    pub const fn http_status(self) -> u16 {
        match self {
            Self::SettingNotFound | Self::CredentialNotFound => 404,
            Self::InvalidSettingValue
            | Self::InvalidSettingKey
            | Self::ValidationFailed
            | Self::CredentialKindScopeMismatch => 400,
            Self::DuplicateCredentialName => 409,
            Self::Unauthorized | Self::UnauthorizedOrganizationOverride => 403,
            Self::UnsupportedNotificationChannel => 422,
        }
    }
}
