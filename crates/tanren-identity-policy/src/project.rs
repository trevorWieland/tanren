//! Project identity newtypes.
//!
//! Domain newtypes for project identifiers and project names. Projects
//! represent exactly one source-control repository registered in Tanren.
//! A project is either account-owned or organization-owned.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Stable identifier for a Tanren project. `UUIDv7` — sortable + unique.
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

/// Maximum char length for a valid project name.
const PROJECT_NAME_MAX_CHARS: usize = 128;
/// Minimum char length for a valid project name.
const PROJECT_NAME_MIN_CHARS: usize = 1;

/// Validated project name.
///
/// Project names identify a registered repository within Tanren. The
/// name is canonicalized: surrounding whitespace is trimmed and inner
/// whitespace is collapsed to a single ASCII space.
///
/// `ProjectName` does NOT derive `Deserialize` — the custom impl below
/// routes every wire input through [`parse`](Self::parse) so blank or
/// malformed names are rejected at the boundary.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String)]
pub struct ProjectName(String);

impl ProjectName {
    /// Parse a raw project name.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::ProjectNameEmpty`](crate::ValidationError::ProjectNameEmpty)
    /// when the input is empty after trimming, or
    /// [`ValidationError::ProjectNameMalformed`](crate::ValidationError::ProjectNameMalformed)
    /// when the normalized name falls outside the allowed shape
    /// (`1..=128` chars, no control characters, and at least one
    /// non-whitespace character).
    pub fn parse(raw: &str) -> Result<Self, crate::ValidationError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(crate::ValidationError::ProjectNameEmpty);
        }

        let normalized: String = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");

        let len = normalized.chars().count();
        if !(PROJECT_NAME_MIN_CHARS..=PROJECT_NAME_MAX_CHARS).contains(&len) {
            return Err(crate::ValidationError::ProjectNameMalformed);
        }
        if normalized.chars().any(char::is_control) {
            return Err(crate::ValidationError::ProjectNameMalformed);
        }

        Ok(Self(normalized))
    }

    /// Borrow the normalized project name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ProjectName {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for ProjectName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
