//! User-tier configuration and credential wire shapes.
//!
//! The schema here intentionally starts small for B-0048/B-0125:
//! - settings: theme + editor preference
//! - credentials: provider/harness token metadata with redacted reads
//!
//! Storage, access control, and handler behavior live in other crates.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tanren_configuration_secrets::UserCredentialMetadata;
pub use tanren_configuration_secrets::{
    ConfigurationValidationFailure, OwnerScope, ThemePreference, UserCredentialId,
    UserCredentialKind, UserCredentialStatus, UserSettingKey, UserSettingValue,
    parse_user_credential_kind, parse_user_setting_key,
};
use tanren_identity_policy::secret_serde;
use utoipa::ToSchema;

/// Upsert request for a user-tier setting.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct UpsertUserSettingRequest {
    /// Setting key.
    pub key: UserSettingKey,
    /// Typed value payload for the key.
    pub value: UserSettingValue,
}

/// Read view for a user-tier setting.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct UserSettingView {
    /// Setting key.
    pub key: UserSettingKey,
    /// Typed setting value.
    pub value: UserSettingValue,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Upsert response for a user-tier setting.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct UpsertUserSettingResponse {
    /// Persisted setting view.
    pub setting: UserSettingView,
}

/// List response for user-tier settings.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListUserSettingsResponse {
    /// Persisted setting views.
    pub items: Vec<UserSettingView>,
    /// Opaque cursor for requesting the next page, if additional rows exist.
    pub next_cursor: Option<String>,
}

/// List request for user-tier settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListUserSettingsRequest {
    /// Maximum rows to return for this page.
    pub limit: Option<u16>,
    /// Opaque pagination cursor from a previous response.
    pub after: Option<String>,
}

/// Remove response for a user-tier setting.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct RemoveUserSettingResponse {
    /// Removed setting view from the pre-delete snapshot.
    pub setting: UserSettingView,
}

/// Create request for a user-owned credential.
#[derive(Debug, Clone, Deserialize, JsonSchema, ToSchema)]
pub struct CreateUserCredentialRequest {
    /// Credential kind.
    pub kind: UserCredentialKind,
    /// Owning scope.
    pub owner_scope: OwnerScope,
    /// Fresh secret value.
    #[serde(deserialize_with = "secret_serde::deserialize_password")]
    #[schemars(with = "String")]
    #[schema(value_type = String, format = Password)]
    pub value: SecretString,
}

/// Update request for an existing user-owned credential.
#[derive(Debug, Clone, Deserialize, JsonSchema, ToSchema)]
pub struct UpdateUserCredentialRequest {
    /// Replacement secret value.
    #[serde(deserialize_with = "secret_serde::deserialize_password")]
    #[schemars(with = "String")]
    #[schema(value_type = String, format = Password)]
    pub value: SecretString,
}

/// Redacted read/list view for a user-owned credential.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct UserCredentialView {
    /// Stable metadata id.
    pub id: UserCredentialId,
    /// Credential kind.
    pub kind: UserCredentialKind,
    /// Owning scope.
    pub owner_scope: OwnerScope,
    /// Lifecycle status.
    pub status: UserCredentialStatus,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

impl From<UserCredentialMetadata> for UserCredentialView {
    fn from(metadata: UserCredentialMetadata) -> Self {
        Self {
            id: metadata.id,
            kind: metadata.kind,
            owner_scope: metadata.owner_scope,
            status: metadata.status,
            created_at: metadata.created_at,
            updated_at: metadata.updated_at,
        }
    }
}

/// Create response for a user-owned credential.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CreateUserCredentialResponse {
    /// Redacted metadata view.
    pub item: UserCredentialView,
}

/// Update response for a user-owned credential.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct UpdateUserCredentialResponse {
    /// Redacted metadata view.
    pub item: UserCredentialView,
}

/// List response for user-owned credentials.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListUserCredentialsResponse {
    /// Redacted metadata rows.
    pub items: Vec<UserCredentialView>,
    /// Opaque cursor for requesting the next page, if additional rows exist.
    pub next_cursor: Option<String>,
}

/// List request for user-owned credentials.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListUserCredentialsRequest {
    /// Maximum rows to return for this page.
    pub limit: Option<u16>,
    /// Opaque pagination cursor from a previous response.
    pub after: Option<String>,
}

/// Remove response for a user-owned credential.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct RemoveUserCredentialResponse {
    /// Removed credential metadata from the pre-delete snapshot.
    pub item: UserCredentialView,
}

/// Capability map for authenticated account-configuration operations.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ConfigurationCapabilitiesView {
    /// Capability set for setting reads and writes.
    pub settings: SettingCapabilitiesView,
    /// Capability set for user-owned secret-item metadata reads and writes.
    pub user_items: CredentialCapabilitiesView,
}

/// Capability set for user-tier settings operations.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SettingCapabilitiesView {
    /// Allowed settings actions for the authenticated actor.
    pub allowed_actions: Vec<SettingCapabilityAction>,
}

/// Per-action capabilities for user-tier settings.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SettingCapabilityAction {
    /// Read current settings.
    Read,
    /// Create or update settings.
    CreateOrUpdate,
    /// Delete settings.
    Delete,
}

/// Capability set for user-owned credential operations.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CredentialCapabilitiesView {
    /// Allowed secret-item actions for the authenticated actor.
    pub allowed_actions: Vec<CredentialCapabilityAction>,
}

/// Per-action capabilities for user-owned secret items.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialCapabilityAction {
    /// Read redacted metadata.
    Read,
    /// Create a new item.
    Create,
    /// Update an existing item.
    Update,
    /// Delete an existing item.
    Delete,
}

/// Response for authenticated configuration capability discovery.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct GetAuthenticatedUserConfigurationCapabilitiesResponse {
    /// Capabilities granted to the authenticated actor.
    pub capabilities: ConfigurationCapabilitiesView,
}

/// Closed taxonomy of user-configuration and user-credential failures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(tag = "code", rename_all = "snake_case")]
#[non_exhaustive]
pub enum UserConfigurationFailureReason {
    /// Contract/domain input failed validation.
    ValidationFailed {
        /// Typed validation detail.
        detail: ConfigurationValidationFailure,
    },
    /// Requested setting key was not present.
    SettingNotFound,
    /// Requested metadata row was not present.
    ItemNotFound,
}

impl UserConfigurationFailureReason {
    /// Stable machine-readable error code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ValidationFailed { .. } => "validation_failed",
            Self::SettingNotFound => "setting_not_found",
            Self::ItemNotFound => "item_not_found",
        }
    }

    /// Human-readable wire summary for this failure.
    #[must_use]
    pub const fn summary(&self) -> &'static str {
        match self {
            Self::ValidationFailed { .. } => {
                "The submitted configuration input did not satisfy contract-level validation."
            }
            Self::SettingNotFound => {
                "The requested user setting does not exist or is not accessible."
            }
            Self::ItemNotFound => {
                "The requested user credential metadata does not exist or is not accessible."
            }
        }
    }

    /// Recommended HTTP status for API/MCP transport projections.
    #[must_use]
    pub const fn http_status(&self) -> u16 {
        match self {
            Self::ValidationFailed { .. } => 400,
            Self::SettingNotFound | Self::ItemNotFound => 404,
        }
    }
}
