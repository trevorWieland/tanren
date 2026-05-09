use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
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
