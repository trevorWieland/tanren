//! Durable organization-level permission names.
//!
//! [`OrganizationPermission`] is the stable newtype that identity-policy owns
//! for the permission axis on organization invitations. M-0004 will layer
//! granular role templates and policy constraints on top; this type
//! intentionally models only the permission name string so the contract
//! surface stays forward-compatible.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::ValidationError;

/// Durable organization-level permission name.
///
/// Constructed via [`OrganizationPermission::parse`] which trims
/// surrounding whitespace and rejects empty strings. The inner value is
/// otherwise opaque — M-0004 will add role-template resolution and policy
/// constraints on top of this raw name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String)]
pub struct OrganizationPermission(String);

impl OrganizationPermission {
    /// Parse a raw permission name. Trims surrounding whitespace and
    /// rejects empty strings.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyPermissionName`] if the input is
    /// empty after trimming.
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ValidationError::EmptyPermissionName);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Borrow the underlying permission name string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for OrganizationPermission {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for OrganizationPermission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
