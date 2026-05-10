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
    ConfigurationValidationFailure, OwnerScope, ThemePreference, USER_CREDENTIAL_KIND_DESCRIPTORS,
    USER_SETTING_DESCRIPTORS, UserCredentialId, UserCredentialKind, UserCredentialKindDescriptor,
    UserCredentialStatus, UserSettingDescriptor, UserSettingKey, UserSettingValue,
    parse_user_credential_kind, parse_user_setting_key, user_credential_kind_wire_name,
    user_setting_key_wire_name,
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

/// Typed payload encoded inside a user-settings list cursor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserSettingsPageCursorPayload {
    /// Cursor kind discriminator.
    pub kind: UserSettingsPageCursorKind,
    /// Last item update timestamp from the current page.
    pub updated_at: DateTime<Utc>,
    /// Last item setting key from the current page.
    pub key: UserSettingKey,
}

/// Discriminator for user-settings list cursor payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserSettingsPageCursorKind {
    /// Cursor payload for settings pagination.
    Settings,
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

/// Typed payload encoded inside a user-credentials list cursor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserCredentialsPageCursorPayload {
    /// Cursor kind discriminator.
    pub kind: UserCredentialsPageCursorKind,
    /// Last item update timestamp from the current page.
    pub updated_at: DateTime<Utc>,
    /// Last item id from the current page.
    pub id: UserCredentialId,
}

/// Discriminator for user-credentials list cursor payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserCredentialsPageCursorKind {
    /// Cursor payload for credential pagination.
    Credentials,
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

/// Stable list of supported user-setting keys for interface prompts/messages.
pub const SUPPORTED_USER_SETTING_KEYS: [&str; USER_SETTING_DESCRIPTORS.len()] =
    supported_user_setting_keys();
/// Stable list of supported credential kinds for interface prompts/messages.
pub const SUPPORTED_USER_CREDENTIAL_KINDS: [&str; USER_CREDENTIAL_KIND_DESCRIPTORS.len()] =
    supported_user_credential_kinds();
/// Stable list of supported theme preferences for interface prompts/messages.
pub const SUPPORTED_THEME_PREFERENCES: [&str; 3] = ["system", "light", "dark"];

const fn supported_user_setting_keys() -> [&'static str; USER_SETTING_DESCRIPTORS.len()] {
    let mut names = [""; USER_SETTING_DESCRIPTORS.len()];
    let mut index = 0;
    while index < USER_SETTING_DESCRIPTORS.len() {
        names[index] = USER_SETTING_DESCRIPTORS[index].wire_name;
        index += 1;
    }
    names
}

const fn supported_user_credential_kinds() -> [&'static str; USER_CREDENTIAL_KIND_DESCRIPTORS.len()]
{
    let mut names = [""; USER_CREDENTIAL_KIND_DESCRIPTORS.len()];
    let mut index = 0;
    while index < USER_CREDENTIAL_KIND_DESCRIPTORS.len() {
        names[index] = USER_CREDENTIAL_KIND_DESCRIPTORS[index].wire_name;
        index += 1;
    }
    names
}

/// Resolve a user-setting key to a stable transport name.
#[must_use]
pub fn user_setting_key_name(key: UserSettingKey) -> &'static str {
    match user_setting_key_wire_name(key) {
        Ok(value) => value,
        Err(_) => "unsupported_setting_key",
    }
}

/// Resolve a credential kind to a stable transport name.
#[must_use]
pub fn user_credential_kind_name(kind: UserCredentialKind) -> &'static str {
    match user_credential_kind_wire_name(kind) {
        Ok(value) => value,
        Err(_) => "unsupported_credential_kind",
    }
}

/// Resolve a credential status to a stable transport name.
#[must_use]
pub const fn user_credential_status_name(status: UserCredentialStatus) -> &'static str {
    match status {
        UserCredentialStatus::Pending => "pending",
        UserCredentialStatus::Active => "active",
        UserCredentialStatus::Invalid => "invalid",
    }
}

/// Resolve a theme preference to a stable transport name.
#[must_use]
pub const fn theme_preference_name(theme: ThemePreference) -> &'static str {
    match theme {
        ThemePreference::System => "system",
        ThemePreference::Light => "light",
        ThemePreference::Dark => "dark",
    }
}

/// Parse a theme preference from its stable transport name.
#[must_use]
pub fn parse_theme_preference(raw: &str) -> Option<ThemePreference> {
    match raw {
        "system" => Some(ThemePreference::System),
        "light" => Some(ThemePreference::Light),
        "dark" => Some(ThemePreference::Dark),
        _ => None,
    }
}

/// Convert shared list-page arguments to a settings list request.
#[must_use]
pub fn user_settings_page_request(
    limit: Option<u16>,
    after: Option<String>,
) -> ListUserSettingsRequest {
    ListUserSettingsRequest { limit, after }
}

/// Convert shared list-page arguments to a credentials list request.
#[must_use]
pub fn user_credentials_page_request(
    limit: Option<u16>,
    after: Option<String>,
) -> ListUserCredentialsRequest {
    ListUserCredentialsRequest { limit, after }
}

/// Parse an optional list-page limit argument from text.
///
/// # Errors
///
/// Returns an error when a non-empty input is not a positive integer.
pub fn parse_optional_page_limit(raw: &str) -> Result<Option<u16>, &'static str> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let parsed = trimmed
        .parse::<u16>()
        .map_err(|_| "limit must be a positive integer")?;
    if parsed == 0 {
        return Err("limit must be a positive integer");
    }
    Ok(Some(parsed))
}

/// Parse an optional list-page cursor argument from text.
#[must_use]
pub fn parse_optional_page_after(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_owned())
}
