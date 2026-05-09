//! Install manifest entry metadata.

use std::path::{Component, Path};

use sha2::{Digest, Sha256};

use crate::install::InstallIntegration;
use crate::install::error::InstallError;

const NIBBLES: &[u8; 16] = b"0123456789abcdef";

/// Installed-asset classification used by install drift and apply planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetClass {
    /// Tanren methodology command material rendered per integration.
    MethodologyCommand,
    /// Standards profile guidance file.
    StandardsProfile,
}

/// Preservation contract for install/update behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Manifest row written/checked by future install workflows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
    pub path: RepoRelativePath,
    pub content_hash: String,
    pub asset_class: AssetClass,
    pub integration: Option<InstallIntegration>,
    pub preservation: PreservationPolicy,
}

/// Convert projected assets into manifest rows.
#[must_use]
pub fn build_manifest_entries(assets: &[InstallAssetProjection]) -> Vec<ManifestEntry> {
    assets
        .iter()
        .map(|asset| ManifestEntry {
            path: asset.destination_path.clone(),
            content_hash: sha256_hex(asset.content),
            asset_class: asset.asset_class,
            integration: asset.integration,
            preservation: asset.preservation,
        })
        .collect()
}

fn sha256_hex(content: &str) -> String {
    let digest = Sha256::digest(content.as_bytes());
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push(char::from(NIBBLES[(byte >> 4) as usize]));
        hex.push(char::from(NIBBLES[(byte & 0x0f) as usize]));
    }
    hex
}
