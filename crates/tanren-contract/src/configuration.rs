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
use tanren_configuration_secrets::{
    ConfigurationValidationFailure, OwnerScope, UserCredentialKind, UserCredentialMetadata,
    UserCredentialStatus, UserSettingKey, UserSettingValue,
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
}

/// Remove response for a user-tier setting.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct RemoveUserSettingResponse {
    /// Removed setting view from the pre-delete snapshot.
    pub setting: UserSettingView,
}

/// Create request for a user-owned credential.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CreateUserCredentialRequest {
    /// Credential kind.
    pub kind: UserCredentialKind,
    /// Owning scope.
    pub owner_scope: OwnerScope,
    /// Fresh secret value.
    #[serde(
        deserialize_with = "secret_serde::deserialize_password",
        serialize_with = "secret_serde::serialize_password_expose"
    )]
    #[schemars(with = "String")]
    #[schema(value_type = String, format = Password)]
    pub value: SecretString,
}

/// Update request for an existing user-owned credential.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct UpdateUserCredentialRequest {
    /// Replacement secret value.
    #[serde(
        deserialize_with = "secret_serde::deserialize_password",
        serialize_with = "secret_serde::serialize_password_expose"
    )]
    #[schemars(with = "String")]
    #[schema(value_type = String, format = Password)]
    pub value: SecretString,
}

/// Redacted read/list view for a user-owned credential.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct UserCredentialView {
    /// Stable metadata id.
    pub id: String,
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
}

/// Remove response for a user-owned credential.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct RemoveUserCredentialResponse {
    /// Removed credential metadata from the pre-delete snapshot.
    pub item: UserCredentialView,
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
