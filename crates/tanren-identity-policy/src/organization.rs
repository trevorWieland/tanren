//! Organization domain types.
//!
//! [`OrganizationName`] and [`OrgPermission`] are the domain primitives used
//! by the organization-creation flow (R-0002) and the bootstrap admin grants
//! held by the creator.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::ValidationError;

/// Validated organization name. Constructed via [`OrganizationName::parse`]
/// which trims surrounding whitespace, rejects empty input, and canonicalises
/// to a case-folded form so differently-cased inputs compare equal.
///
/// # Wire-input contract
///
/// `OrganizationName` does NOT derive `Deserialize` — the custom impl below
/// routes every wire input through [`parse`](Self::parse). Without this,
/// `#[serde(transparent)]` would let HTTP/MCP/CLI requests carry untrimmed
/// or differently-cased names, leading to duplicate organization names that
/// differ only in case.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String)]
pub struct OrganizationName(String);

impl OrganizationName {
    /// Parse a raw organization name. Trims surrounding whitespace and
    /// case-folds to Unicode lowercase so case variants of the same name
    /// compare equal.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyOrganizationName`] if the input is
    /// empty after trimming.
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ValidationError::EmptyOrganizationName);
        }
        Ok(Self(trimmed.to_lowercase()))
    }

    /// Borrow the canonical (trimmed + case-folded) organization name.
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

/// Administrative permission granted to an organization member.
///
/// The bootstrap creator of an organization receives all permissions. Later
/// M-0004 introduces granular roles that compose from these permissions
/// without renaming them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum OrgPermission {
    /// Invite new members to the organization.
    InviteMembers,
    /// Manage member access levels and roles.
    ManageAccess,
    /// Configure organization settings and metadata.
    Configure,
    /// Set organization-wide policies.
    SetPolicy,
    /// Delete the organization.
    Delete,
}
