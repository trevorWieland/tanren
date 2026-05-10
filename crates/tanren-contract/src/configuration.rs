//! User-tier configuration and credential wire shapes.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tanren_configuration_secrets::UserCredentialMetadata;
pub use tanren_configuration_secrets::{
    ConfigurationValidationFailure, EditorSetting, OwnerScope, ThemePreference,
    USER_CREDENTIAL_KIND_DESCRIPTORS, USER_SETTING_DESCRIPTORS, UserCredentialId,
    UserCredentialKind, UserCredentialKindDescriptor, UserCredentialStatus, UserSettingDescriptor,
    UserSettingKey, UserSettingValue, parse_user_credential_kind, parse_user_setting_key,
    user_credential_kind_wire_name, user_setting_key_wire_name,
};
use tanren_identity_policy::secret_serde;
use utoipa::ToSchema;
/// Upsert request for a user-tier setting.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpsertUserSettingRequest {
    pub key: UserSettingKey,
    pub value: UserSettingValue,
}
/// Read view for a user-tier setting.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct UserSettingView {
    pub key: UserSettingKey,
    pub value: UserSettingValue,
    pub updated_at: DateTime<Utc>,
}
/// Upsert response for a user-tier setting.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct UpsertUserSettingResponse {
    pub setting: UserSettingView,
}
/// List response for user-tier settings.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListUserSettingsResponse {
    pub items: Vec<UserSettingView>,
    /// Opaque cursor for requesting the next page, if additional rows exist.
    pub next_cursor: Option<String>,
}
/// List request for user-tier settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ListUserSettingsRequest {
    pub limit: Option<u16>,
    pub after: Option<String>,
}
/// Typed payload encoded inside a user-settings list cursor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserSettingsPageCursorPayload {
    pub kind: UserSettingsPageCursorKind,
    pub updated_at: DateTime<Utc>,
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
    pub setting: UserSettingView,
}
/// Create request for a user-owned credential.
#[derive(Debug, Clone, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateUserCredentialRequest {
    pub kind: UserCredentialKind,
    pub owner_scope: OwnerScope,
    /// Fresh secret value.
    #[serde(deserialize_with = "secret_serde::deserialize_password")]
    #[schemars(with = "String")]
    #[schema(value_type = String, format = Password)]
    pub value: SecretString,
}
/// Update request for an existing user-owned credential.
#[derive(Debug, Clone, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
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
    pub id: UserCredentialId,
    pub kind: UserCredentialKind,
    pub owner_scope: OwnerScope,
    pub status: UserCredentialStatus,
    pub created_at: DateTime<Utc>,
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
    pub item: UserCredentialView,
}
/// Update response for a user-owned credential.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct UpdateUserCredentialResponse {
    pub item: UserCredentialView,
}
/// List response for user-owned credentials.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListUserCredentialsResponse {
    pub items: Vec<UserCredentialView>,
    /// Opaque cursor for requesting the next page, if additional rows exist.
    pub next_cursor: Option<String>,
}
/// List request for user-owned credentials.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ListUserCredentialsRequest {
    pub limit: Option<u16>,
    pub after: Option<String>,
}
/// Typed payload encoded inside a user-credentials list cursor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserCredentialsPageCursorPayload {
    pub kind: UserCredentialsPageCursorKind,
    pub updated_at: DateTime<Utc>,
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
    pub item: UserCredentialView,
}
/// Version discriminator for configuration capability responses.
///
/// Callers check this to evolve client logic alongside the contract surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ConfigurationVersion(u32);
impl ConfigurationVersion {
    /// Current configuration contract version.
    pub const CURRENT: Self = Self(1);
    /// Construct from numeric form.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
    /// Numeric value.
    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }
}
/// Closed registry of setting capability actions.
pub const SETTING_CAPABILITY_ACTIONS: [SettingCapabilityAction; 3] = [
    SettingCapabilityAction::Read,
    SettingCapabilityAction::CreateOrUpdate,
    SettingCapabilityAction::Delete,
];
/// Closed registry of credential capability actions.
pub const CREDENTIAL_CAPABILITY_ACTIONS: [CredentialCapabilityAction; 4] = [
    CredentialCapabilityAction::Read,
    CredentialCapabilityAction::Create,
    CredentialCapabilityAction::Update,
    CredentialCapabilityAction::Delete,
];
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
    /// Allowed settings actions derived from the capability registry.
    pub allowed_actions: Vec<SettingCapabilityAction>,
}
/// Per-action capabilities for user-tier settings.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, ToSchema, PartialEq, Eq)]
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
    /// Allowed credential actions derived from the capability registry.
    pub allowed_actions: Vec<CredentialCapabilityAction>,
}
/// Per-action capabilities for user-owned secret items.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, ToSchema, PartialEq, Eq)]
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
/// Registry-driven builder for configuration capability views.
///
/// Derives [`ConfigurationCapabilitiesView`] from the closed
/// [`SETTING_CAPABILITY_ACTIONS`] / [`CREDENTIAL_CAPABILITY_ACTIONS`]
/// registries rather than ad-hoc hard-coded action vectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigurationCapabilityRegistry {
    setting_flags: [bool; SETTING_CAPABILITY_ACTIONS.len()],
    cred_flags: [bool; CREDENTIAL_CAPABILITY_ACTIONS.len()],
}
impl ConfigurationCapabilityRegistry {
    /// Full capabilities for an authenticated account session.
    #[must_use]
    pub const fn account() -> Self {
        Self {
            setting_flags: [true; SETTING_CAPABILITY_ACTIONS.len()],
            cred_flags: [true; CREDENTIAL_CAPABILITY_ACTIONS.len()],
        }
    }
    /// Deny all configuration mutations.
    #[must_use]
    pub const fn denied() -> Self {
        Self {
            setting_flags: [false; SETTING_CAPABILITY_ACTIONS.len()],
            cred_flags: [false; CREDENTIAL_CAPABILITY_ACTIONS.len()],
        }
    }
    /// Build the wire view from the registry state.
    #[must_use]
    pub fn to_view(self) -> ConfigurationCapabilitiesView {
        ConfigurationCapabilitiesView {
            settings: SettingCapabilitiesView {
                allowed_actions: SETTING_CAPABILITY_ACTIONS
                    .iter()
                    .zip(self.setting_flags)
                    .filter_map(|(&action, include)| include.then_some(action))
                    .collect(),
            },
            user_items: CredentialCapabilitiesView {
                allowed_actions: CREDENTIAL_CAPABILITY_ACTIONS
                    .iter()
                    .zip(self.cred_flags)
                    .filter_map(|(&action, include)| include.then_some(action))
                    .collect(),
            },
        }
    }
    /// Check whether a specific setting action is enabled.
    #[must_use]
    pub fn has_setting_action(self, action: SettingCapabilityAction) -> bool {
        SETTING_CAPABILITY_ACTIONS
            .iter()
            .zip(self.setting_flags)
            .any(|(&registered, enabled)| registered == action && enabled)
    }

    /// Check whether a specific credential action is enabled.
    #[must_use]
    pub fn has_credential_action(self, action: CredentialCapabilityAction) -> bool {
        CREDENTIAL_CAPABILITY_ACTIONS
            .iter()
            .zip(self.cred_flags)
            .any(|(&registered, enabled)| registered == action && enabled)
    }
}
/// Response for authenticated configuration capability discovery.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct GetAuthenticatedUserConfigurationCapabilitiesResponse {
    /// Configuration contract version for this response shape.
    pub version: ConfigurationVersion,
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
/// Stable list of supported user-setting keys.
pub const SUPPORTED_USER_SETTING_KEYS: [&str; USER_SETTING_DESCRIPTORS.len()] =
    supported_user_setting_keys();
/// Stable list of supported credential kinds for interface prompts/messages.
pub const SUPPORTED_USER_CREDENTIAL_KINDS: [&str; USER_CREDENTIAL_KIND_DESCRIPTORS.len()] =
    supported_user_credential_kinds();
/// Stable list of supported theme preferences for interface prompts/messages.
pub const SUPPORTED_THEME_PREFERENCES: [&str; 3] = ["system", "light", "dark"];
/// Maximum allowed page size for configuration list requests.
pub const MAX_PAGE_LIMIT: u16 = 100;
/// Validated page-size bound. Guarantees the inner `u16` is in `1..=MAX_PAGE_LIMIT`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(try_from = "u16", into = "u16")]
pub struct BoundedPageLimit(u16);
impl BoundedPageLimit {
    /// Reject zero or values above [`MAX_PAGE_LIMIT`].
    pub fn new(value: u16) -> Result<Self, &'static str> {
        if value == 0 || value > MAX_PAGE_LIMIT {
            return Err("limit must be between 1 and 100");
        }
        Ok(Self(value))
    }
    /// The validated inner value.
    #[must_use]
    pub const fn into_inner(self) -> u16 {
        self.0
    }
}
impl TryFrom<u16> for BoundedPageLimit {
    type Error = &'static str;
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<BoundedPageLimit> for u16 {
    fn from(value: BoundedPageLimit) -> Self {
        value.into_inner()
    }
}
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
