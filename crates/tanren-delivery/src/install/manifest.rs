//! Install manifest entry metadata.

use std::fmt;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Component, Path};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::install::InstallIntegration;
use crate::install::InstallProfile;
use crate::install::error::InstallError;

const NIBBLES: &[u8; 16] = b"0123456789abcdef";
const SHA256_HEX_LENGTH: usize = 64;
const SHA256_STREAM_BUFFER_SIZE: usize = 16 * 1024;

/// Install manifest schema version.
pub(super) const INSTALL_MANIFEST_VERSION: ManifestVersion = ManifestVersion::new(1);
/// Repo-local metadata path for persisted install state.
pub(super) const INSTALL_MANIFEST_REPO_PATH: &str = ".tanren/install-manifest.toml";

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

/// Install manifest schema version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ManifestVersion(u32);

impl ManifestVersion {
    /// Build a manifest version value.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Numeric version value used in serialized manifests.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

impl fmt::Display for ManifestVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Serialize for ManifestVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u32(self.as_u32())
    }
}

impl<'de> Deserialize<'de> for ManifestVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        u32::deserialize(deserializer).map(Self::new)
    }
}

/// Canonical repository-relative path (no absolute roots, no `..` traversal).
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

        let mut normalized = Vec::new();
        for component in candidate.components() {
            match component {
                Component::Normal(segment) => {
                    normalized.push(segment.to_string_lossy().into_owned());
                }
                Component::CurDir => {}
                _ => {
                    return Err(InstallError::InvalidRepoRelativePath {
                        path: path.to_owned(),
                    });
                }
            }
        }

        if normalized.is_empty() {
            return Err(InstallError::InvalidRepoRelativePath {
                path: path.to_owned(),
            });
        }

        Ok(Self(normalized.join("/")))
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
pub(super) struct InstallAssetProjection {
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

/// Parse error for [`Sha256Hex`].
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("content hash must be exactly {SHA256_HEX_LENGTH} lowercase hex characters")]
pub struct Sha256HexParseError;

impl Sha256Hex {
    /// Validate and construct a SHA-256 hex digest.
    pub fn parse(value: &str) -> Result<Self, Sha256HexParseError> {
        if value.len() != SHA256_HEX_LENGTH
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(Sha256HexParseError);
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
    pub manifest_version: ManifestVersion,
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
pub(super) fn build_manifest_entries(assets: &[InstallAssetProjection]) -> Vec<ManifestEntry> {
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
pub(super) fn sha256_hex(bytes: &[u8]) -> Sha256Hex {
    let digest = Sha256::digest(bytes);
    sha256_hex_from_digest(digest.as_ref())
}

/// Hash a file as lowercase SHA-256 hex using buffered streaming reads.
pub(super) fn sha256_hex_file(path: &Path) -> Result<Sha256Hex, std::io::Error> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; SHA256_STREAM_BUFFER_SIZE];

    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    let digest = hasher.finalize();
    Ok(sha256_hex_from_digest(digest.as_ref()))
}

fn sha256_hex_from_digest(digest: &[u8]) -> Sha256Hex {
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push(char::from(NIBBLES[(byte >> 4) as usize]));
        hex.push(char::from(NIBBLES[(byte & 0x0f) as usize]));
    }
    Sha256Hex(hex)
}
