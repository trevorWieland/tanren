use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::ValidationError;

const PROJECT_ID_VERSION: usize = 7;

const REPOSITORY_REF_MAX_TOTAL_LEN: usize = 140;
const REPOSITORY_OWNER_MAX_LEN: usize = 39;
const REPOSITORY_NAME_MAX_LEN: usize = 100;

const PROVIDER_FAMILY_MIN_LEN: usize = 2;
const PROVIDER_FAMILY_MAX_LEN: usize = 48;

const DESIGNATED_HOST_MIN_LEN: usize = 1;
const DESIGNATED_HOST_MAX_LEN: usize = 253;
const DESIGNATED_HOST_LABEL_MAX_LEN: usize = 63;

/// Stable identifier for a Tanren project. `UUIDv7` — sortable + unique.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String, format = "uuid")]
pub struct ProjectId(Uuid);

impl ProjectId {
    /// Allocate a fresh time-ordered id.
    #[must_use]
    pub fn fresh() -> Self {
        Self(Uuid::now_v7())
    }

    /// Parse a string UUID and validate Tanren's project-id invariants.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::ProjectIdInvalid`] when the input is
    /// not a `UUIDv7` value.
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let parsed = Uuid::parse_str(raw).map_err(|_| ValidationError::ProjectIdInvalid)?;
        Self::try_from_uuid(parsed)
    }

    /// Validate a raw UUID for use as a project id.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::ProjectIdInvalid`] when the UUID is
    /// not version 7.
    pub fn try_from_uuid(value: Uuid) -> Result<Self, ValidationError> {
        if value.get_version_num() != PROJECT_ID_VERSION {
            return Err(ValidationError::ProjectIdInvalid);
        }
        Ok(Self(value))
    }

    /// The underlying UUID.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
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

impl<'de> Deserialize<'de> for ProjectId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = Uuid::deserialize(d)?;
        Self::try_from_uuid(raw).map_err(serde::de::Error::custom)
    }
}

/// Canonical repository identity (`owner/name`) used for project setup.
///
/// The value is lower-cased + trimmed during parse so different case
/// spellings of the same repository map to one canonical key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(
    value_type = String,
    pattern = "^[a-z0-9](?:[a-z0-9-]{0,37}[a-z0-9])?/[a-z0-9](?:[a-z0-9._-]{0,98}[a-z0-9])?$"
)]
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
        if trimmed.len() > REPOSITORY_REF_MAX_TOTAL_LEN {
            return Err(ValidationError::RepositoryRefInvalid);
        }
        let normalized = trimmed.to_lowercase();
        let mut parts = normalized.split('/');
        let owner = parts.next().ok_or(ValidationError::RepositoryRefInvalid)?;
        let name = parts.next().ok_or(ValidationError::RepositoryRefInvalid)?;
        if parts.next().is_some()
            || owner.is_empty()
            || name.is_empty()
            || owner.len() > REPOSITORY_OWNER_MAX_LEN
            || name.len() > REPOSITORY_NAME_MAX_LEN
            || !repository_owner_valid(owner)
            || !repository_name_valid(name)
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

/// Stable identifier for a source-control provider family.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(
    value_type = String,
    pattern = "^(?!.*--)[a-z0-9][a-z0-9-]{0,46}[a-z0-9]$"
)]
pub struct ProviderFamily(String);

impl ProviderFamily {
    /// Canonical source-control provider family slug used by baseline adapters.
    #[must_use]
    pub fn source_control() -> Self {
        Self("fixture-source-control".to_owned())
    }

    /// Parse a provider family slug.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::ProviderFamilyInvalid`] when the slug
    /// is not ASCII lowercase-kebab-case or exceeds bounds.
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let normalized = raw.trim().to_ascii_lowercase();
        if normalized.len() < PROVIDER_FAMILY_MIN_LEN
            || normalized.len() > PROVIDER_FAMILY_MAX_LEN
            || !slug_valid(&normalized)
        {
            return Err(ValidationError::ProviderFamilyInvalid);
        }
        Ok(Self(normalized))
    }

    /// Borrow the provider family slug.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ProviderFamily {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for ProviderFamily {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Validated designated source-control host key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(
    value_type = String,
    pattern = "^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)(?:\\.(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?))*$"
)]
pub struct DesignatedHost(String);

impl DesignatedHost {
    /// Parse a host key from user or wire input.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::DesignatedHostInvalid`] when the host
    /// fails allowed-label rules or exceeds bounds.
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let normalized = raw.trim().to_ascii_lowercase();
        if normalized.len() < DESIGNATED_HOST_MIN_LEN || normalized.len() > DESIGNATED_HOST_MAX_LEN
        {
            return Err(ValidationError::DesignatedHostInvalid);
        }
        if normalized.starts_with('.') || normalized.ends_with('.') {
            return Err(ValidationError::DesignatedHostInvalid);
        }
        if normalized
            .split('.')
            .any(|label| !designated_host_label_valid(label))
        {
            return Err(ValidationError::DesignatedHostInvalid);
        }
        Ok(Self(normalized))
    }

    /// Borrow the designated host value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for DesignatedHost {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for DesignatedHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn repository_owner_valid(value: &str) -> bool {
    if value.starts_with('-') || value.ends_with('-') || value.contains("--") {
        return false;
    }
    value
        .bytes()
        .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'-'))
}

fn repository_name_valid(value: &str) -> bool {
    if value.starts_with('-')
        || value.starts_with('.')
        || value.starts_with('_')
        || value.ends_with('-')
        || value.ends_with('.')
        || value.ends_with('_')
    {
        return false;
    }
    value.bytes().all(|b| {
        matches!(
            b,
            b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-'
        )
    })
}

fn slug_valid(value: &str) -> bool {
    if value.starts_with('-') || value.ends_with('-') || value.contains("--") {
        return false;
    }
    value
        .bytes()
        .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'-'))
}

fn designated_host_label_valid(label: &str) -> bool {
    if label.is_empty()
        || label.len() > DESIGNATED_HOST_LABEL_MAX_LEN
        || label.starts_with('-')
        || label.ends_with('-')
    {
        return false;
    }
    label
        .bytes()
        .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'-'))
}
