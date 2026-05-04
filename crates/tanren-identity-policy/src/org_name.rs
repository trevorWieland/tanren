//! Organization name newtype and bootstrap admin permissions.
//!
//! [`OrgName`] is the validated, case-normalised display name for an
//! organization. Uniqueness is *globally unique, case-insensitive* — the
//! lower-case canonical form stored here is what the persistence layer
//! indexes with a `UNIQUE` constraint so two case variants of the same
//! logical name cannot coexist.
//!
//! [`OrgAdminPermissions`] enumerates the five administrative capabilities
//! the org creator receives at bootstrap. Granular role / policy
//! configuration is M-0004's scope; this type captures only the initial
//! set.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{IdentityError, ValidationError};

/// Maximum byte length of a valid organization name.
const ORG_NAME_MAX_LEN: usize = 100;

/// Validated organization display name.
///
/// Constructed via [`OrgName::parse`] which trims surrounding whitespace,
/// rejects empty / over-length inputs, and normalises to lower-case so
/// case variants of the same name compare equal.
///
/// # Uniqueness rule
///
/// Globally unique, case-insensitive (normalised). The persistence layer
/// enforces this with a `UNIQUE` index on the stored lower-case value.
///
/// # Wire-input contract
///
/// `OrgName` does NOT derive `Deserialize` — the custom impl below
/// routes every wire input through [`parse`](Self::parse) so untrimmed
/// or upper-case names cannot bypass canonicalisation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String)]
pub struct OrgName(String);

impl OrgName {
    /// Parse a raw organization name.
    ///
    /// Trims surrounding whitespace, rejects empty or over-length inputs,
    /// and normalises to lower-case for case-insensitive uniqueness.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityError::Validation`] wrapping
    /// [`ValidationError::EmptyOrgName`] when the input is empty
    /// after trimming, or [`ValidationError::OrgNameTooLong`] when the
    /// trimmed input exceeds 100 bytes.
    pub fn parse(raw: &str) -> Result<Self, IdentityError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ValidationError::EmptyOrgName.into());
        }
        if trimmed.len() > ORG_NAME_MAX_LEN {
            return Err(ValidationError::OrgNameTooLong.into());
        }
        Ok(Self(trimmed.to_lowercase()))
    }

    /// Borrow the canonical (trimmed + lower-cased) name string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for OrgName {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for OrgName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Bootstrap administrative permissions granted to the organization
/// creator.
///
/// The creator receives the full set at org-creation time. These
/// permissions may be delegated or revoked through the organization
/// policy subsystem (M-0004). The invariant that at least one member
/// holds administrative permissions is enforced by the leave/remove
/// flows in R-0007.
///
/// Serialized as a `u8` bitmask — each flag corresponds to one of
/// the associated constants (`INVITE`, `MANAGE_ACCESS`, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = u8)]
pub struct OrgAdminPermissions(u8);

impl OrgAdminPermissions {
    /// Permission to send invitations to new members.
    pub const INVITE: u8 = 1 << 0;
    /// Permission to grant or revoke member access.
    pub const MANAGE_ACCESS: u8 = 1 << 1;
    /// Permission to change organization-level configuration.
    pub const CONFIGURE: u8 = 1 << 2;
    /// Permission to set organization policy.
    pub const SET_POLICY: u8 = 1 << 3;
    /// Permission to delete the organization.
    pub const DELETE: u8 = 1 << 4;

    /// Return the full permission set granted to the organization creator
    /// at bootstrap.
    #[must_use]
    pub fn bootstrap_creator() -> Self {
        Self(Self::INVITE | Self::MANAGE_ACCESS | Self::CONFIGURE | Self::SET_POLICY | Self::DELETE)
    }

    /// Returns `true` when no permissions are held.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns `true` when the given flag is set.
    #[must_use]
    pub fn contains(self, flag: u8) -> bool {
        self.0 & flag != 0
    }

    /// Set the given flag, returning the updated permissions.
    #[must_use]
    pub fn insert(mut self, flag: u8) -> Self {
        self.0 |= flag;
        self
    }

    /// Clear the given flag, returning the updated permissions.
    #[must_use]
    pub fn remove(mut self, flag: u8) -> Self {
        self.0 &= !flag;
        self
    }
}
