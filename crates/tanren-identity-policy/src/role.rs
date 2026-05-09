use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{AccountId, OrgId};

/// Maximum byte length of a valid role name.
const ROLE_NAME_MAX_LEN: usize = 64;
/// Maximum byte length of a valid permission name.
const PERMISSION_NAME_MAX_LEN: usize = 120;

/// Stable identifier for a Tanren project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String, format = "uuid")]
pub struct ProjectId(Uuid);

impl ProjectId {
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

impl From<Uuid> for ProjectId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl AsRef<Uuid> for ProjectId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl std::fmt::Display for ProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Stable identifier for a role template.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String, format = "uuid")]
pub struct RoleId(Uuid);

impl RoleId {
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

impl From<Uuid> for RoleId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl AsRef<Uuid> for RoleId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl std::fmt::Display for RoleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Stable identifier for an individual permission grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String, format = "uuid")]
pub struct PermissionGrantId(Uuid);

impl PermissionGrantId {
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

impl From<Uuid> for PermissionGrantId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl AsRef<Uuid> for PermissionGrantId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl std::fmt::Display for PermissionGrantId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Errors raised when parsing role/permission value types.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RoleValueError {
    /// The supplied role name was empty after trimming.
    #[error("role name is empty")]
    EmptyRoleName,
    /// The supplied role name exceeded the maximum allowed length.
    #[error("role name is too long")]
    RoleNameTooLong,
    /// The supplied role name contained an unsupported character.
    #[error("role name contains an invalid character")]
    RoleNameInvalidChar,
    /// The supplied permission name was empty after trimming.
    #[error("permission name is empty")]
    EmptyPermissionName,
    /// The supplied permission name exceeded the maximum allowed length.
    #[error("permission name is too long")]
    PermissionNameTooLong,
    /// The supplied permission name contained an unsupported character.
    #[error("permission name contains an invalid character")]
    PermissionNameInvalidChar,
}

/// Role template name.
///
/// `RoleName` does not derive `Deserialize`; the custom impl validates and
/// canonicalizes wire input through [`parse`](Self::parse).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String)]
pub struct RoleName(String);

impl RoleName {
    /// Parse a role name from raw caller input.
    ///
    /// # Errors
    ///
    /// Returns [`RoleValueError::EmptyRoleName`] when the value is empty after
    /// trimming, [`RoleValueError::RoleNameTooLong`] when it exceeds the
    /// maximum length, or [`RoleValueError::RoleNameInvalidChar`] when it
    /// contains unsupported characters.
    pub fn parse(raw: &str) -> Result<Self, RoleValueError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(RoleValueError::EmptyRoleName);
        }
        if trimmed.len() > ROLE_NAME_MAX_LEN {
            return Err(RoleValueError::RoleNameTooLong);
        }
        if !trimmed.chars().all(is_valid_role_name_char) {
            return Err(RoleValueError::RoleNameInvalidChar);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Borrow the role name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for RoleName {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for RoleName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Permission identifier.
///
/// `PermissionName` does not derive `Deserialize`; the custom impl validates
/// and canonicalizes wire input through [`parse`](Self::parse).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String)]
pub struct PermissionName(String);

impl PermissionName {
    /// Parse a permission name from raw caller input.
    ///
    /// # Errors
    ///
    /// Returns [`RoleValueError::EmptyPermissionName`] when the value is empty
    /// after trimming, [`RoleValueError::PermissionNameTooLong`] when it
    /// exceeds the maximum length, or
    /// [`RoleValueError::PermissionNameInvalidChar`] when it contains
    /// unsupported characters.
    pub fn parse(raw: &str) -> Result<Self, RoleValueError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(RoleValueError::EmptyPermissionName);
        }
        if trimmed.len() > PERMISSION_NAME_MAX_LEN {
            return Err(RoleValueError::PermissionNameTooLong);
        }
        let canonical = trimmed.to_ascii_lowercase();
        if !canonical.chars().all(is_valid_permission_name_char) {
            return Err(RoleValueError::PermissionNameInvalidChar);
        }
        let starts_or_ends_with_separator =
            matches!(canonical.chars().next(), Some('.' | ':' | '-'))
                || matches!(canonical.chars().next_back(), Some('.' | ':' | '-'));
        if starts_or_ends_with_separator || canonical.contains("..") || canonical.contains("::") {
            return Err(RoleValueError::PermissionNameInvalidChar);
        }
        Ok(Self(canonical))
    }

    /// Borrow the permission name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for PermissionName {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for PermissionName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn is_valid_role_name_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '.' | '/')
}

fn is_valid_permission_name_char(ch: char) -> bool {
    ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '.' | ':' | '_' | '-')
}

/// Scope where role templates can be created and resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum RoleScope {
    /// Role scoped to one account.
    Account { account_id: AccountId },
    /// Role scoped to one organization.
    Organization { org_id: OrgId },
    /// Role scoped to one project.
    Project { project_id: ProjectId },
}

/// Scope where permission grants apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum PermissionScope {
    /// Permission grant scoped to one account.
    Account { account_id: AccountId },
    /// Permission grant scoped to one organization.
    Organization { org_id: OrgId },
    /// Permission grant scoped to one project.
    Project { project_id: ProjectId },
}

/// Role template identity paired with its scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ScopedRole {
    /// Stable role template id.
    pub role_id: RoleId,
    /// Scope where the role template is defined.
    pub scope: RoleScope,
}

/// Permission identity paired with its scope.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ScopedPermission {
    /// Permission name.
    pub permission: PermissionName,
    /// Scope where the permission is granted or evaluated.
    pub scope: PermissionScope,
}

/// Principal reference used by permission-grant and permission-check flows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(tag = "principal", rename_all = "snake_case")]
pub enum PrincipalRef {
    /// Human account principal.
    Account { account_id: AccountId },
    /// Role identifiers are intentionally rejected as authorization
    /// principals by higher layers.
    Role { role_id: RoleId },
}
