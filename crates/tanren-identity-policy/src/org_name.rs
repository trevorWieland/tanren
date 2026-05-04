//! `OrgName` validated newtype and `OrgAdminPermissions` bootstrap flags.
//!
//! Split out of `lib.rs` to keep the crate under the workspace 500-line
//! budget. See module-level docs in `lib.rs` for the broader identity-policy
//! surface.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{IdentityError, ValidationError};

/// Maximum byte length of an organization name.
const ORG_NAME_MAX_LEN: usize = 100;

/// Validated, canonical organisation display name.
///
/// Constructed via [`OrgName::parse`] which trims surrounding whitespace,
/// rejects empty / over-length inputs, and normalises to lower-case so that
/// two case variants of the same logical name compare equal.
///
/// # Uniqueness rule
///
/// Organisation names are **globally unique, case-insensitive**. The
/// lower-cased canonical form is what the persistence layer stores in its
/// `UNIQUE` index; callers should compare via [`OrgName::as_str`] (already
/// normalised) rather than the raw input.
///
/// # Wire-input contract
///
/// `OrgName` does **not** derive `Deserialize` — the custom impl below
/// routes every wire input through [`parse`](Self::parse) so untrimmed or
/// differently-cased names cannot bypass canonicalisation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String)]
pub struct OrgName(String);

impl OrgName {
    /// Parse a raw organisation name. Trims, validates length, and
    /// canonicalises to lower-case.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityError::Validation`] wrapping
    /// [`ValidationError::EmptyOrgName`] when the input is empty after
    /// trimming, or [`ValidationError::OrgNameTooLong`] when it exceeds
    /// the maximum allowed length.
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

    /// Borrow the canonical (trimmed + lower-cased) organisation name.
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

/// Bootstrap administrative permissions held by the organisation creator.
///
/// Covers the five capabilities the creator receives at org-creation time.
/// Each flag is independent; future role / policy work (M-0004) will derive
/// these from a richer permission model. For now the flat struct is the
/// authoritative representation of the "last admin holder cannot remove
/// themselves" invariant source data.
///
/// Serialises as a self-describing JSON object so API / MCP consumers can
/// inspect individual flags without decoding a bitmask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrgAdminPermissions {
    /// Invite new members to the organisation.
    pub invite: bool,
    /// Grant or revoke member access levels.
    pub manage_access: bool,
    /// Change organisation-level settings (display name, metadata).
    pub configure: bool,
    /// Set organisation policy (allowed auth methods, joining rules).
    pub set_policy: bool,
    /// Delete the organisation (requires being the last admin holder).
    pub delete: bool,
}

impl OrgAdminPermissions {
    /// Return the full permission set granted to an organisation creator at
    /// bootstrap time.
    #[must_use]
    pub const fn bootstrap_creator() -> Self {
        Self {
            invite: true,
            manage_access: true,
            configure: true,
            set_policy: true,
            delete: true,
        }
    }

    /// Whether all bootstrap permissions are present.
    #[must_use]
    pub fn is_full(self) -> bool {
        self.invite && self.manage_access && self.configure && self.set_policy && self.delete
    }

    /// Whether no permissions are present.
    #[must_use]
    pub fn is_empty(self) -> bool {
        !self.invite && !self.manage_access && !self.configure && !self.set_policy && !self.delete
    }

    /// Encode the five boolean flags into a `u32` bitfield.
    ///
    /// Bit mapping (matches the persistence-layer comment in
    /// `tanren_store::records::MembershipRecord::permissions`):
    ///
    /// | bit | flag           |
    /// |-----|----------------|
    /// | 0   | invite         |
    /// | 1   | manage_access  |
    /// | 2   | configure      |
    /// | 3   | set_policy     |
    /// | 4   | delete         |
    #[must_use]
    pub fn to_bits(self) -> u32 {
        let mut bits = 0u32;
        if self.invite {
            bits |= 1 << 0;
        }
        if self.manage_access {
            bits |= 1 << 1;
        }
        if self.configure {
            bits |= 1 << 2;
        }
        if self.set_policy {
            bits |= 1 << 3;
        }
        if self.delete {
            bits |= 1 << 4;
        }
        bits
    }

    /// Decode a `u32` bitfield back into the five boolean flags.
    /// Unknown bits are silently ignored.
    #[must_use]
    pub fn from_bits(bits: u32) -> Self {
        Self {
            invite: (bits & (1 << 0)) != 0,
            manage_access: (bits & (1 << 1)) != 0,
            configure: (bits & (1 << 2)) != 0,
            set_policy: (bits & (1 << 3)) != 0,
            delete: (bits & (1 << 4)) != 0,
        }
    }
}
