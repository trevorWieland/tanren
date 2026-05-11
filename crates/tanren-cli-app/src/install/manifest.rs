//! Install manifest entry metadata.
//!
//! Re-exports canonical install contract types from [`tanren_contract::install`]
//! at `pub(crate)` visibility so sibling install modules can import them via
//! `crate::install::manifest`.

pub(crate) use tanren_contract::install::{
    AssetClass, INSTALL_MANIFEST_REPO_PATH, INSTALL_MANIFEST_VERSION, InstallManifest,
    ManifestEntry, PreservationPolicy, RepoRelativePath, Sha256Hex, sha256_file_streaming,
    sha256_hex,
};

use crate::install::InstallIntegration;

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
