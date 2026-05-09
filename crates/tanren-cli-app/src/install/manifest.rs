//! Install manifest entry metadata.

use std::fmt;
use std::path::{Component, Path};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};

use crate::install::InstallIntegration;
use crate::install::InstallProfile;
use crate::install::error::InstallError;

const NIBBLES: &[u8; 16] = b"0123456789abcdef";
const SHA256_HEX_LENGTH: usize = 64;

/// Install manifest schema version.
pub const INSTALL_MANIFEST_VERSION: u32 = 1;
/// Repo-local metadata path for persisted install state.
pub const INSTALL_MANIFEST_REPO_PATH: &str = ".tanren/install-manifest.toml";

/// Installed-asset classification used by install drift and apply planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssetClass {
    /// Tanren methodology command material rendered per integration.
    MethodologyCommand,
    /// Standards profile guidance file.
    StandardsProfile,
}

/// Preservation contract for install/update behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PreservationPolicy {
    /// Tanren-owned generated file that can be replaced on re-install.
    ReplaceGenerated,
    /// User-editable standards file that should not be overwritten silently.
    PreserveUserEdits,
}

/// Strict repository-relative path (no absolute roots, no `..` traversal).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepoRelativePath(String);

impl RepoRelativePath {
    /// Validate and construct a repository-relative path.
    pub fn parse(path: &str) -> Result<Self, InstallError> {
        if path.is_empty() {
            return Err(InstallError::InvalidRepoRelativePath {
                path: path.to_owned(),
            });
        }

        let candidate = Path::new(path);
        if candidate.is_absolute() {
            return Err(InstallError::InvalidRepoRelativePath {
                path: path.to_owned(),
            });
        }

        let is_valid = candidate
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir));
        if !is_valid {
            return Err(InstallError::InvalidRepoRelativePath {
                path: path.to_owned(),
            });
        }

        Ok(Self(path.to_owned()))
    }

    /// Borrow the validated path string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Borrow as a [`Path`].
    #[must_use]
    pub fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }
}

impl Serialize for RepoRelativePath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RepoRelativePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        RepoRelativePath::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// Catalog asset entry before it becomes a file-write plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallAssetProjection {
    pub source_path: RepoRelativePath,
    pub destination_path: RepoRelativePath,
    pub content: &'static str,
    pub asset_class: AssetClass,
    pub integration: Option<InstallIntegration>,
    pub preservation: PreservationPolicy,
}

/// Strict lowercase SHA-256 digest encoded as 64 hex characters.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sha256Hex(String);

impl Sha256Hex {
    /// Validate and construct a SHA-256 hex digest.
    pub fn parse(value: &str) -> Result<Self, String> {
        if value.len() != SHA256_HEX_LENGTH
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(format!(
                "content hash must be exactly {SHA256_HEX_LENGTH} lowercase hex characters"
            ));
        }

        Ok(Self(value.to_owned()))
    }

    /// Borrow the validated digest string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Sha256Hex {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for Sha256Hex {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Sha256Hex {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Sha256Hex::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// Manifest row written/checked by future install workflows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub path: RepoRelativePath,
    pub content_hash: Sha256Hex,
    pub asset_class: AssetClass,
    pub integration: Option<InstallIntegration>,
    pub preservation: PreservationPolicy,
}

/// Install manifest stored under repo-local metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallManifest {
    pub manifest_version: u32,
    pub profile: InstallProfile,
    pub integrations: Vec<InstallIntegration>,
    pub entries: Vec<ManifestEntry>,
}

impl InstallManifest {
    /// Create a deterministic manifest from typed install inputs.
    #[must_use]
    pub fn from_entries(
        profile: InstallProfile,
        integrations: Vec<InstallIntegration>,
        entries: Vec<ManifestEntry>,
    ) -> Self {
        Self {
            manifest_version: INSTALL_MANIFEST_VERSION,
            profile,
            integrations,
            entries,
        }
    }
}

/// Convert projected assets into manifest rows.
#[must_use]
pub fn build_manifest_entries(assets: &[InstallAssetProjection]) -> Vec<ManifestEntry> {
    assets
        .iter()
        .map(|asset| ManifestEntry {
            path: asset.destination_path.clone(),
            content_hash: sha256_hex(asset.content.as_bytes()),
            asset_class: asset.asset_class,
            integration: asset.integration,
            preservation: asset.preservation,
        })
        .collect()
}

/// Hash bytes as lowercase SHA-256 hex.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> Sha256Hex {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push(char::from(NIBBLES[(byte >> 4) as usize]));
        hex.push(char::from(NIBBLES[(byte & 0x0f) as usize]));
    }
    Sha256Hex(hex)
}
