use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use utoipa::ToSchema;

use crate::ValidationError;

/// Maximum char length for a valid organization name.
const ORGANIZATION_NAME_MAX_CHARS: usize = 64;
/// Minimum char length for a valid organization name.
const ORGANIZATION_NAME_MIN_CHARS: usize = 3;

/// Validated organization name.
///
/// Organization names are canonicalized to a global uniqueness key:
/// surrounding whitespace is trimmed, inner whitespace is collapsed to a
/// single ASCII space, and letters are lower-cased. Persistence and service
/// layers should use [`OrganizationName::as_str`] as the uniqueness value so
/// case/whitespace variants cannot produce duplicate organizations.
///
/// `OrganizationName` does NOT derive `Deserialize` — the custom impl below
/// routes every wire input through [`parse`](Self::parse) so blank/malformed
/// names are rejected at the boundary with stable validation errors.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String)]
pub struct OrganizationName(String);

impl OrganizationName {
    /// Parse a raw organization name.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::OrganizationNameEmpty`] when the input is
    /// empty after trimming. Returns
    /// [`ValidationError::OrganizationNameMalformed`] when the normalized name
    /// falls outside the allowed shape (`3..=64` chars, ASCII
    /// alphanumeric/space/`-`/`_`/`.` only, and at least one alphanumeric
    /// character).
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ValidationError::OrganizationNameEmpty);
        }

        let normalized = trimmed
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();

        let len = normalized.chars().count();
        if !(ORGANIZATION_NAME_MIN_CHARS..=ORGANIZATION_NAME_MAX_CHARS).contains(&len) {
            return Err(ValidationError::OrganizationNameMalformed);
        }
        let has_alnum = normalized.chars().any(|ch| ch.is_ascii_alphanumeric());
        let valid_charset = normalized.chars().all(|ch| {
            ch.is_ascii_lowercase()
                || ch.is_ascii_digit()
                || ch == ' '
                || ch == '-'
                || ch == '_'
                || ch == '.'
        });
        if !has_alnum || !valid_charset {
            return Err(ValidationError::OrganizationNameMalformed);
        }

        Ok(Self(normalized))
    }

    /// Borrow the normalized organization name (the global uniqueness key).
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for OrganizationName {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for OrganizationName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Stable idempotency key for replay-safe mutation requests.
///
/// The value is preserved except for surrounding whitespace trimming so
/// existing database rows remain valid without shape migrations.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String)]
pub struct IdempotencyKey(String);

impl IdempotencyKey {
    /// Parse a raw idempotency key.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::IdempotencyKeyEmpty`] if the input is
    /// empty after trimming.
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ValidationError::IdempotencyKeyEmpty);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Borrow the normalized idempotency key.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for IdempotencyKey {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for IdempotencyKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Closed set of organization-level administrative permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationPermission {
    /// Invite people into the organization.
    Invite,
    /// Grant/revoke organization-level access for existing members.
    ManageAccess,
    /// Configure organization-level shared defaults.
    Configure,
    /// Manage organization-level policy (approval, governance).
    SetPolicy,
    /// Delete/disband the organization.
    Delete,
}

impl OrganizationPermission {
    /// Canonical ordered set of all organization permissions.
    pub const ALL: [Self; 5] = [
        Self::Invite,
        Self::ManageAccess,
        Self::Configure,
        Self::SetPolicy,
        Self::Delete,
    ];

    /// Canonical wire/storage key for the permission.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Invite => "invite",
            Self::ManageAccess => "manage_access",
            Self::Configure => "configure",
            Self::SetPolicy => "set_policy",
            Self::Delete => "delete",
        }
    }
}

impl std::fmt::Display for OrganizationPermission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for OrganizationPermission {
    type Err = ParseOrganizationPermissionError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let normalized = raw.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "invite" => Ok(Self::Invite),
            "manage_access" => Ok(Self::ManageAccess),
            "configure" => Ok(Self::Configure),
            "set_policy" => Ok(Self::SetPolicy),
            "delete" => Ok(Self::Delete),
            _ => Err(ParseOrganizationPermissionError {
                value: raw.to_owned(),
            }),
        }
    }
}

/// Error returned when parsing an organization permission key fails.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid organization permission key: {value}")]
pub struct ParseOrganizationPermissionError {
    value: String,
}
