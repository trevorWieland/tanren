//! Configuration and Secrets subsystem.
//!
//! Configuration is tier-scoped (user, account, project, organization) with
//! deterministic inheritance. Secret values are encrypted at rest and never
//! recorded in event payloads, projection files, or proof artifacts; only
//! non-secret metadata is event-replayable.

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
        "invalid standards root path '{path}': must be non-empty, relative, and not include parent traversal"
    )]
    InvalidStandardsRootPath { path: String },
    /// TOML serialization failed.
    #[error("failed to serialize project methodology configuration as TOML: {0}")]
    ProjectMethodologyTomlSerialize(#[from] toml::ser::Error),
    /// TOML deserialization failed.
    #[error("failed to parse project methodology configuration from TOML: {0}")]
    ProjectMethodologyTomlDeserialize(#[from] toml::de::Error),
}

/// Project methodology contract schema version.
pub const PROJECT_METHODOLOGY_SCHEMA_VERSION: u32 = 1;

/// Supported standards methodology profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MethodologyProfile {
    /// Rust + Cargo standards profile.
    RustCargo,
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

        let is_valid = candidate
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir));
        if !is_valid {
            return Err(ConfigSecretsError::InvalidStandardsRootPath {
                path: path.to_owned(),
            });
        }

        Ok(Self(normalized.to_owned()))
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
    pub schema_version: u32,
    /// Standards methodology profile selected for this project.
    pub profile: MethodologyProfile,
    /// Repository-relative root directory containing adopted standards.
    pub standards_root: StandardsRoot,
}

impl ProjectMethodologyConfig {
    /// Build a typed project methodology config from raw inputs.
    pub fn new(
        schema_version: u32,
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
