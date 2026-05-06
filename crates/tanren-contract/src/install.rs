//! Install manifest wire shapes.
//!
//! Typed contract surface for the Tanren bootstrap/install flow. These types
//! model the persisted install manifest (`.tanren/install-manifest.json`) and
//! the validation surface for profile names, integration names, managed paths,
//! and content hashes. Invalid states cannot be constructed through public
//! constructors — every newtype routes construction through a `parse` or `new`
//! method that enforces invariants.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use utoipa::ToSchema;

const MANIFEST_VERSION_CURRENT: u32 = 1;

const KNOWN_PROFILES: &[&str] = &["default", "react-ts-pnpm", "rust-cargo"];

const KNOWN_INTEGRATIONS: &[&str] = &["claude", "codex", "opencode"];

const CONTENT_HEX_LEN: usize = 64;

/// Version of the install manifest schema.
///
/// Bumped when the persisted shape changes incompatibly. Independent of
/// [`ContractVersion`](crate::ContractVersion).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct InstallManifestVersion(u32);

impl InstallManifestVersion {
    /// Current manifest schema version.
    pub const CURRENT: Self = Self(MANIFEST_VERSION_CURRENT);

    /// Construct a manifest version from its numeric form.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// The numeric value of this manifest version.
    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }
}

/// Validated standards profile name.
///
/// Constructed via [`ProfileName::parse`] which checks the name against the
/// closed set of known profiles. Unknown names are rejected at the contract
/// boundary before any files are written.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
pub struct ProfileName(String);

impl ProfileName {
    /// Parse a profile name against the known set.
    ///
    /// # Errors
    ///
    /// Returns [`InstallContractError::UnknownProfile`] when the name is not
    /// in the known set.
    pub fn parse(raw: &str) -> Result<Self, InstallContractError> {
        let trimmed = raw.trim();
        if KNOWN_PROFILES.contains(&trimmed) {
            Ok(Self(trimmed.to_owned()))
        } else {
            Err(InstallContractError::UnknownProfile {
                name: trimmed.to_owned(),
                known: KNOWN_PROFILES.iter().map(|s| (*s).to_owned()).collect(),
            })
        }
    }

    /// Borrow the profile name string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ProfileName {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for ProfileName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Validated agent integration name.
///
/// Constructed via [`IntegrationName::parse`] which checks the name against
/// the closed set of known integrations. Unknown names are rejected at the
/// contract boundary before any files are written.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
pub struct IntegrationName(String);

impl IntegrationName {
    /// Parse an integration name against the known set.
    ///
    /// # Errors
    ///
    /// Returns [`InstallContractError::UnknownIntegration`] when the name is
    /// not in the known set.
    pub fn parse(raw: &str) -> Result<Self, InstallContractError> {
        let trimmed = raw.trim();
        if KNOWN_INTEGRATIONS.contains(&trimmed) {
            Ok(Self(trimmed.to_owned()))
        } else {
            Err(InstallContractError::UnknownIntegration {
                name: trimmed.to_owned(),
                known: KNOWN_INTEGRATIONS.iter().map(|s| (*s).to_owned()).collect(),
            })
        }
    }

    /// Borrow the integration name string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for IntegrationName {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for IntegrationName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Validated SHA-256 content hash.
///
/// Exactly 64 lowercase hexadecimal characters. Constructed via
/// [`ContentHash::parse`] which enforces length and character set.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
pub struct ContentHash(String);

impl ContentHash {
    /// Parse a content hash string.
    ///
    /// # Errors
    ///
    /// Returns [`InstallContractError::InvalidContentHash`] when the string
    /// is not exactly 64 lowercase hexadecimal characters.
    pub fn parse(raw: &str) -> Result<Self, InstallContractError> {
        if raw.len() == CONTENT_HEX_LEN && raw.bytes().all(|b| b.is_ascii_hexdigit()) {
            Ok(Self(raw.to_ascii_lowercase()))
        } else {
            Err(InstallContractError::InvalidContentHash)
        }
    }

    /// Construct from a hex digest computed programmatically (e.g. by a
    /// SHA-256 hasher).
    ///
    /// Semantically equivalent to [`ContentHash::parse`] but named to signal
    /// that the caller produced the digest rather than accepting user input.
    ///
    /// # Errors
    ///
    /// Returns [`InstallContractError::InvalidContentHash`] when the string
    /// is not exactly 64 lowercase hexadecimal characters.
    pub fn from_digest(hex: &str) -> Result<Self, InstallContractError> {
        Self::parse(hex)
    }

    /// Borrow the hash hex string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ContentHash {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// Validated managed path.
///
/// A relative path within the target repository. Must be non-empty, must not
/// start with `/`, and must not contain `..` components. Constructed via
/// [`ManagedPath::parse`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
pub struct ManagedPath(String);

impl ManagedPath {
    /// Parse a managed path.
    ///
    /// # Errors
    ///
    /// Returns [`InstallContractError::InvalidManagedPath`] when the path is
    /// empty, absolute, or contains `..` components.
    pub fn parse(raw: &str) -> Result<Self, InstallContractError> {
        if raw.is_empty() || raw.starts_with('/') {
            return Err(InstallContractError::InvalidManagedPath);
        }
        let has_dotdot = std::path::Path::new(raw)
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir));
        if has_dotdot {
            return Err(InstallContractError::InvalidManagedPath);
        }
        Ok(Self(raw.to_owned()))
    }

    /// Borrow the path string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ManagedPath {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// Discriminator for the kind of managed asset.
///
/// Controls reinstall behaviour: generated assets are replaced on reinstall;
/// standard assets are preserved when user-edited (content hash differs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    /// Generated asset (rendered command or integration file). Replaced
    /// unconditionally on reinstall.
    Generated,
    /// Standard asset (methodology standard file). Preserved on reinstall
    /// when the user has edited the content.
    Standard,
}

/// A single file entry in the install manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ManifestFileEntry {
    /// Relative path within the target repository.
    pub path: ManagedPath,
    /// SHA-256 content hash at install time.
    pub content_hash: ContentHash,
    /// Whether this asset is generated or a standard.
    pub asset_kind: AssetKind,
}

/// The persisted install manifest.
///
/// Written to `.tanren/install-manifest.json` after a successful install.
/// All fields are typed so that deserialisation rejects invalid states.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct InstallManifest {
    /// Schema version of this manifest.
    pub version: InstallManifestVersion,
    /// Standards profile used for this install.
    pub profile: ProfileName,
    /// Agent integrations selected for this install.
    pub integrations: Vec<IntegrationName>,
    /// File entries managed by this install.
    pub entries: Vec<ManifestFileEntry>,
    /// Wall-clock time this manifest was written.
    pub installed_at: DateTime<Utc>,
}

/// Errors raised when install contract validation fails.
#[derive(Debug, Clone, Error)]
#[non_exhaustive]
pub enum InstallContractError {
    /// The profile name is not in the known set.
    #[error("unknown profile: {name}, known: {}", known.join(", "))]
    UnknownProfile {
        /// The rejected name.
        name: String,
        /// The known set at the time of validation.
        known: Vec<String>,
    },
    /// The integration name is not in the known set.
    #[error("unknown integration: {name}, known: {}", known.join(", "))]
    UnknownIntegration {
        /// The rejected name.
        name: String,
        /// The known set at the time of validation.
        known: Vec<String>,
    },
    /// The content hash is not a valid 64-char lowercase hex string.
    #[error("content hash must be exactly 64 lowercase hex characters")]
    InvalidContentHash,
    /// The managed path is empty, absolute, or contains `..`.
    #[error("managed path must be a non-empty relative path without .. components")]
    InvalidManagedPath,
}
