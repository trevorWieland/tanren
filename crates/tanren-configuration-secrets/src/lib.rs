//! Configuration and Secrets subsystem.
//!
//! Configuration is tier-scoped (user, account, project, organization) with
//! deterministic inheritance. Secret values are encrypted at rest and never
//! recorded in event payloads, projection files, or proof artifacts; only
//! non-secret metadata is event-replayable.
//!
//! ## Organization secrets
//!
//! Organization-secret identifiers, names, lifecycle status, and baseline use
//! policy are explicit types so callers, handlers, and stores cannot confuse
//! metadata identifiers with raw secret values. Secret values are always
//! [`SecretString`]; response types never expose a value field.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;
use tanren_identity_policy::OrgId;
use thiserror::Error;
use utoipa::ToSchema;
use uuid::Uuid;

/// Configuration tiers in inheritance order, most-specific to most-general.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Tier {
    /// User-tier configuration. Most specific.
    User,
    /// Project-tier configuration.
    Project,
    /// Account-tier configuration.
    Account,
    /// Organization-tier configuration. Most general.
    Organization,
}

/// Stable identifier for an organization secret. `UUIDv7` — time-ordered.
///
/// Metadata identifier, not a secret value. Safe to log, serialize, display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String, format = "uuid")]
pub struct OrganizationSecretId(Uuid);

impl OrganizationSecretId {
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

impl From<Uuid> for OrganizationSecretId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl AsRef<Uuid> for OrganizationSecretId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl std::fmt::Display for OrganizationSecretId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Maximum char length for a valid organization secret name.
const SECRET_NAME_MAX_CHARS: usize = 128;
/// Minimum char length for a valid organization secret name.
const SECRET_NAME_MIN_CHARS: usize = 1;

/// Validated name for an organization secret.
///
/// Names are unique within an organization, canonicalized: surrounding
/// whitespace trimmed, inner whitespace collapsed to a single ASCII
/// underscore, letters lower-cased. Metadata identifier, not a secret value.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String)]
pub struct OrganizationSecretName(String);

impl OrganizationSecretName {
    /// Parse a raw organization secret name.
    ///
    /// # Errors
    ///
    /// Returns [`SecretNameValidationError::Empty`] when empty after trimming,
    /// or [`SecretNameValidationError::Malformed`] when outside allowed shape
    /// (`1..=128` chars, ASCII alphanumeric / `-` / `_` only).
    pub fn parse(raw: &str) -> Result<Self, SecretNameValidationError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(SecretNameValidationError::Empty);
        }
        let normalized = trimmed
            .split_whitespace()
            .collect::<Vec<_>>()
            .join("_")
            .to_lowercase();
        let len = normalized.chars().count();
        if !(SECRET_NAME_MIN_CHARS..=SECRET_NAME_MAX_CHARS).contains(&len) {
            return Err(SecretNameValidationError::Malformed);
        }
        let valid = normalized
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_');
        if !valid {
            return Err(SecretNameValidationError::Malformed);
        }
        Ok(Self(normalized))
    }
    /// Borrow the normalized secret name (uniqueness key within an org).
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for OrganizationSecretName {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for OrganizationSecretName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Errors raised when constructing an [`OrganizationSecretName`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum SecretNameValidationError {
    /// The supplied name was empty after trimming.
    #[error("organization secret name is empty")]
    Empty,
    /// The supplied name did not satisfy naming rules.
    #[error("organization secret name is malformed")]
    Malformed,
}

/// The scope that owns a secret. Organization secrets are always
/// organization-scoped; this enum is the owner-scope discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SecretOwnerScope {
    /// Owned by an organization.
    Organization,
}

impl SecretOwnerScope {
    /// Serialize to the snake-case string stored in the database.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Organization => "organization",
        }
    }
}

impl std::str::FromStr for SecretOwnerScope {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "organization" => Ok(Self::Organization),
            _ => Err(()),
        }
    }
}

/// Lifecycle status of an organization secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SecretLifecycleStatus {
    /// A value has been written and the secret is usable.
    Active,
    /// The secret has been soft-deleted and is no longer usable.
    Retired,
    /// A rotation is in progress; the old value is still valid.
    Rotating,
}

impl SecretLifecycleStatus {
    /// Serialize to the snake-case string stored in the database.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Retired => "retired",
            Self::Rotating => "rotating",
        }
    }
}

impl std::str::FromStr for SecretLifecycleStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(Self::Active),
            "retired" => Ok(Self::Retired),
            "rotating" => Ok(Self::Rotating),
            _ => Err(()),
        }
    }
}

/// Baseline use policy governing which actors may use a secret's value.
///
/// Narrow baseline model: `MemberUse` allows organization members to use
/// the secret through authorized subsystems; `AdminOnly` restricts use to
/// actors with organization `Configure` permission. Granular policy
/// configuration (B-0233) is a future concern and not modeled here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BaselineUsePolicy {
    /// Organization members may use the secret through authorized subsystems.
    MemberUse,
    /// Only actors with organization admin-level permission may use it.
    AdminOnly,
}

impl BaselineUsePolicy {
    /// Serialize to the snake-case string stored in the database.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MemberUse => "member_use",
            Self::AdminOnly => "admin_only",
        }
    }
}

impl std::str::FromStr for BaselineUsePolicy {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "member_use" => Ok(Self::MemberUse),
            "admin_only" => Ok(Self::AdminOnly),
            _ => Err(()),
        }
    }
}

/// Monotonically increasing version number for a secret's value.
///
/// The invariant "version >= 1" is encoded at the type level via
/// [`NonZeroU32`], so zero cannot be represented without explicit
/// unsafe code. Deserialization rejects zero at the boundary.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema, ToSchema,
)]
#[serde(transparent)]
#[schema(value_type = u32)]
pub struct SecretVersion(NonZeroU32);

impl SecretVersion {
    /// Initial version for a newly created secret.
    pub const INITIAL: Self = Self(NonZeroU32::MIN);

    /// Fallibly construct a version from a raw value.
    ///
    /// Returns `None` when `value` is zero — versions are monotonic from 1.
    #[must_use]
    pub const fn try_new(value: u32) -> Option<Self> {
        match NonZeroU32::new(value) {
            Some(nz) => Some(Self(nz)),
            None => None,
        }
    }

    /// The underlying version number, guaranteed >= 1.
    #[must_use]
    pub const fn value(self) -> u32 {
        self.0.get()
    }

    /// Increment to the next version.
    ///
    /// Returns `None` on overflow past `u32::MAX`. In practice, version
    /// numbers are far below this limit.
    #[must_use]
    pub fn next(self) -> Option<Self> {
        self.0
            .get()
            .checked_add(1)
            .and_then(NonZeroU32::new)
            .map(Self)
    }
}

impl std::fmt::Display for SecretVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Replayable metadata for a stored secret. The secret value itself is held
/// out-of-band by an encrypted store and never appears in this record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretMetadata {
    /// Stable secret identifier (slug, not value).
    pub id: String,
    /// Owning configuration tier.
    pub tier: Tier,
    /// Provider or harness this secret is associated with, if any.
    pub provider: Option<String>,
    /// True once a value has been written; false until the first set.
    pub present: bool,
}

/// Rich metadata view for an organization secret. Domain-level projection
/// used by app-services and store layers. Contains no secret value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationSecretMetadata {
    /// Stable secret identifier.
    pub id: OrganizationSecretId,
    /// Owning organization.
    pub org_id: OrgId,
    /// Validated secret name (unique within the organization).
    pub name: OrganizationSecretName,
    /// Owner scope discriminator.
    pub owner_scope: SecretOwnerScope,
    /// Current lifecycle status.
    pub status: SecretLifecycleStatus,
    /// Baseline use policy.
    pub use_policy: BaselineUsePolicy,
    /// Monotonically increasing version of the stored value.
    pub version: SecretVersion,
    /// Optional description.
    pub description: Option<String>,
    /// Optional provider.
    pub provider: Option<String>,
    /// When the secret was created.
    pub created_at: DateTime<Utc>,
    /// When the metadata was last updated.
    pub updated_at: DateTime<Utc>,
}

/// Holder for a freshly-resolved secret value. The wrapper zeroes on drop.
#[derive(Debug, Clone)]
pub struct ResolvedSecret {
    /// Identifier this value resolved against.
    pub id: String,
    /// The secret value. Zeroed on drop via [`secrecy::SecretString`].
    pub value: SecretString,
}

/// Errors raised by configuration and secrets operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConfigSecretsError {
    /// Lookup found no value at any tier.
    #[error("no value found for key '{0}'")]
    NotFound(String),
    /// Secret name validation failed.
    #[error("secret name validation failed")]
    NameValidation(#[from] SecretNameValidationError),
}
