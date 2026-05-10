//! Configuration and Secrets subsystem.
//!
//! Configuration is tier-scoped (user, account, project, organization) with
//! deterministic inheritance. Secret values are encrypted at rest and never
//! recorded in event payloads, projection files, or proof artifacts; only
//! non-secret metadata is event-replayable.

use std::fmt;
use std::path::{Component, Path};

use secrecy::SecretString;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Configuration tiers in inheritance order, from most-specific to most-general.
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
    /// Standards-root path is not a valid repo-relative path.
    #[error(
        "invalid standards root path '{path}': must be non-empty, relative, and contain only normal path segments"
    )]
    InvalidStandardsRootPath { path: String },
    /// TOML serialization failed.
    #[error("failed to serialize project methodology configuration as TOML: {0}")]
    ProjectMethodologyTomlSerialize(#[from] toml::ser::Error),
    /// TOML deserialization failed.
    #[error("failed to parse project methodology configuration from TOML: {0}")]
    ProjectMethodologyTomlDeserialize(#[from] toml::de::Error),
    /// Config schema version is not supported by this runtime.
    #[error(
        "unsupported project methodology schema version {actual}; supported version is {supported}"
    )]
    UnsupportedProjectMethodologySchemaVersion {
        actual: ProjectMethodologySchemaVersion,
        supported: ProjectMethodologySchemaVersion,
    },
}

/// Project methodology contract schema version.
pub const PROJECT_METHODOLOGY_SCHEMA_VERSION: ProjectMethodologySchemaVersion =
    ProjectMethodologySchemaVersion::new(1);

/// Project methodology contract schema version wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectMethodologySchemaVersion(u32);

impl ProjectMethodologySchemaVersion {
    /// Construct a schema version from a raw number.
    #[must_use]
    pub const fn new(version: u32) -> Self {
        Self(version)
    }

    /// Borrow this schema version as a raw integer.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }

    /// Validate compatibility with this runtime's supported schema version.
    pub fn ensure_supported(self) -> Result<(), ConfigSecretsError> {
        if self == PROJECT_METHODOLOGY_SCHEMA_VERSION {
            return Ok(());
        }
        Err(
            ConfigSecretsError::UnsupportedProjectMethodologySchemaVersion {
                actual: self,
                supported: PROJECT_METHODOLOGY_SCHEMA_VERSION,
            },
        )
    }
}

impl fmt::Display for ProjectMethodologySchemaVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Setting families for typed effective-configuration resolution metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveConfigurationSettingFamily {
    /// Standards methodology profile adoption.
    StandardsProfile,
    /// Standards repository root selection.
    StandardsRoot,
}

/// Configuration source scope for an effective setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveConfigurationSourceScope {
    /// User-scoped configuration.
    User,
    /// Account-scoped configuration.
    Account,
    /// Organization-scoped configuration.
    Organization,
    /// Project-scoped configuration.
    Project,
    /// Service-account scoped configuration.
    ServiceAccount,
    /// Assignment-scoped configuration.
    Assignment,
    /// Installation-scoped configuration.
    Installation,
}

/// Resolution shape for an effective setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveConfigurationResolutionKind {
    /// Declared directly at the source scope.
    Explicit,
    /// Inherited from an upstream scope.
    Inherited,
    /// Applied from a default.
    Defaulted,
    /// Overridden by a more specific setting.
    Overridden,
    /// Locked by policy.
    Locked,
}

/// Policy constraint that governed the effective result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveConfigurationPolicyConstraint {
    /// No additional constraint applied.
    None,
}

/// Whether the effective value is currently usable for the active actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveConfigurationActorUsability {
    /// Usable by the current actor.
    Usable,
    /// Not usable by the current actor.
    NotUsable,
}

/// Freshness state for an effective-configuration projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveConfigurationFreshness {
    /// Projection is current for the inspected repository snapshot.
    Current,
    /// Projection may be stale.
    Stale,
}

/// Resolution metadata attached to an effective configuration field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectiveConfigurationMetadata {
    /// Setting family associated with this resolved field.
    pub setting_family: EffectiveConfigurationSettingFamily,
    /// Scope where the effective value originated.
    pub source_scope: EffectiveConfigurationSourceScope,
    /// How the value became effective.
    pub resolution_kind: EffectiveConfigurationResolutionKind,
    /// Policy constraint applied during resolution.
    pub policy_constraint: EffectiveConfigurationPolicyConstraint,
    /// Whether the value is usable by the active actor.
    pub actor_usability: EffectiveConfigurationActorUsability,
    /// Freshness state of the projection that produced this value.
    pub freshness: EffectiveConfigurationFreshness,
    /// Projection position, when available.
    pub projection_position: Option<u64>,
}

impl EffectiveConfigurationMetadata {
    /// Create project-scoped explicit metadata for a standards setting.
    #[must_use]
    pub const fn project_explicit(setting_family: EffectiveConfigurationSettingFamily) -> Self {
        Self {
            setting_family,
            source_scope: EffectiveConfigurationSourceScope::Project,
            resolution_kind: EffectiveConfigurationResolutionKind::Explicit,
            policy_constraint: EffectiveConfigurationPolicyConstraint::None,
            actor_usability: EffectiveConfigurationActorUsability::Usable,
            freshness: EffectiveConfigurationFreshness::Current,
            projection_position: None,
        }
    }
}

/// Supported standards methodology profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MethodologyProfile {
    /// Rust + Cargo standards profile.
    RustCargo,
}

impl MethodologyProfile {
    /// Canonical methodology profile identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RustCargo => "rust-cargo",
        }
    }
}

/// Strict repository-relative standards root (no absolute roots, no `..` traversal).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StandardsRoot(String);

impl StandardsRoot {
    /// Validate and construct a repository-relative standards root path.
    pub fn parse(path: &str) -> Result<Self, ConfigSecretsError> {
        let normalized = path.trim();
        if normalized.is_empty() {
            return Err(ConfigSecretsError::InvalidStandardsRootPath {
                path: path.to_owned(),
            });
        }

        let candidate = Path::new(normalized);
        if candidate.is_absolute() {
            return Err(ConfigSecretsError::InvalidStandardsRootPath {
                path: path.to_owned(),
            });
        }

        let mut canonical_segments = Vec::new();
        for component in candidate.components() {
            match component {
                Component::Normal(segment) => {
                    canonical_segments.push(segment.to_string_lossy().into_owned());
                }
                Component::CurDir
                | Component::ParentDir
                | Component::RootDir
                | Component::Prefix(_) => {
                    return Err(ConfigSecretsError::InvalidStandardsRootPath {
                        path: path.to_owned(),
                    });
                }
            }
        }

        if canonical_segments.is_empty() {
            return Err(ConfigSecretsError::InvalidStandardsRootPath {
                path: path.to_owned(),
            });
        }

        Ok(Self(canonical_segments.join("/")))
    }

    /// Borrow the standards root as a string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Borrow the standards root as a path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        Path::new(self.as_str())
    }
}

impl Serialize for StandardsRoot {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for StandardsRoot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        StandardsRoot::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// Project-tier methodology configuration persisted as repo-projected TOML.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectMethodologyConfig {
    /// Contract schema version for compatibility checks.
    pub schema_version: ProjectMethodologySchemaVersion,
    /// Standards methodology profile selected for this project.
    pub profile: MethodologyProfile,
    /// Repository-relative root directory containing adopted standards.
    pub standards_root: StandardsRoot,
}

impl ProjectMethodologyConfig {
    /// Build a typed project methodology config from raw inputs.
    pub fn new(
        schema_version: ProjectMethodologySchemaVersion,
        profile: MethodologyProfile,
        standards_root: &str,
    ) -> Result<Self, ConfigSecretsError> {
        let standards_root = StandardsRoot::parse(standards_root)?;
        Ok(Self {
            schema_version,
            profile,
            standards_root,
        })
    }

    /// Parse project methodology configuration from TOML.
    pub fn from_toml(toml_text: &str) -> Result<Self, ConfigSecretsError> {
        toml::from_str(toml_text).map_err(ConfigSecretsError::from)
    }

    /// Serialize project methodology configuration as TOML.
    pub fn to_toml(&self) -> Result<String, ConfigSecretsError> {
        toml::to_string(self).map_err(ConfigSecretsError::from)
    }
}
