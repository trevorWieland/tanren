use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::ValidationError;

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

/// Canonical repository identity (`owner/name`) used for project setup.
///
/// The value is lower-cased + trimmed during parse so different case
/// spellings of the same repository map to one canonical key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String, pattern = "^[a-z0-9._-]+/[a-z0-9._-]+$")]
pub struct RepositoryRef(String);

impl RepositoryRef {
    /// Parse a raw repository identity into canonical `owner/name` form.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::RepositoryRefEmpty`] if the input is
    /// empty after trimming, or
    /// [`ValidationError::RepositoryRefInvalid`] if the input is not a
    /// two-part `owner/name` value with allowed ASCII chars.
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ValidationError::RepositoryRefEmpty);
        }
        let normalized = trimmed.to_lowercase();
        let mut parts = normalized.split('/');
        let owner = parts.next().ok_or(ValidationError::RepositoryRefInvalid)?;
        let name = parts.next().ok_or(ValidationError::RepositoryRefInvalid)?;
        if parts.next().is_some()
            || owner.is_empty()
            || name.is_empty()
            || !repository_part_valid(owner)
            || !repository_part_valid(name)
        {
            return Err(ValidationError::RepositoryRefInvalid);
        }
        Ok(Self(normalized))
    }

    /// Borrow the canonical repository identity.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for RepositoryRef {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for RepositoryRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn repository_part_valid(value: &str) -> bool {
    value
        .bytes()
        .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-'))
}
