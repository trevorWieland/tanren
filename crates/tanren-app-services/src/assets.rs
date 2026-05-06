//! Asset upgrade preview planner.
//!
//! Reads the installed `.tanren/asset-manifest` from a repository root,
//! compares it against the embedded current asset bundle, and produces an
//! [`UpgradePreviewResponse`] describing what would change. The preview is
//! read-only — no files are written.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tanren_contract::{
    AssetAction, AssetEntry, AssetManifest, AssetOwnership, MANIFEST_FORMAT_VERSION,
    MigrationConcern, MigrationConcernKind, UpgradePreviewResponse,
};

/// Error taxonomy for the asset upgrade preview flow.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PreviewError {
    /// The provided root directory does not exist.
    #[error("root directory does not exist: {0}")]
    RootNotFound(PathBuf),
    /// The asset manifest file is missing.
    #[error("asset manifest not found at {0}")]
    ManifestMissing(PathBuf),
    /// The manifest file could not be parsed.
    #[error("failed to parse asset manifest: {0}")]
    ManifestParse(String),
    /// The manifest format version is unsupported.
    #[error("unsupported manifest version {manifest} (supported: {supported})")]
    UnsupportedVersion { manifest: u32, supported: u32 },
}

/// A single asset in the embedded current bundle.
#[derive(Debug, Clone)]
pub struct BundledAsset {
    /// Relative path from the repository root.
    pub path: PathBuf,
    /// Content hash in the form `sha256:<hex>`.
    pub hash: String,
}

/// The embedded current asset bundle — the set of assets the running Tanren
/// version would install.
#[derive(Debug, Clone)]
pub struct AssetBundle {
    /// Target Tanren version this bundle represents.
    pub version: String,
    /// Assets in this bundle.
    pub assets: Vec<BundledAsset>,
}

/// Returns the embedded current asset bundle for this Tanren version.
///
/// In a full implementation this would be generated at build time from
/// actual template files. The preview slice embeds a representative set
/// so the upgrade preview flow is exercisable end-to-end.
#[must_use]
pub fn current_bundle() -> AssetBundle {
    AssetBundle {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        assets: vec![
            BundledAsset {
                path: PathBuf::from(".tanren/config.toml"),
                hash: "sha256:aaa111bbb222ccc333ddd444eee555fff666777888999000aaabbbcccddd1112"
                    .to_owned(),
            },
            BundledAsset {
                path: PathBuf::from("commands/check.md"),
                hash: "sha256:bbb222ccc333ddd444eee555fff666777888999000aaabbbcccddd111aaa2223"
                    .to_owned(),
            },
            BundledAsset {
                path: PathBuf::from("commands/build.md"),
                hash: "sha256:ccc333ddd444eee555fff666777888999000aaabbbcccddd111aaa222bbb3334"
                    .to_owned(),
            },
        ],
    }
}

/// Produce an upgrade preview for the repository at `root`.
///
/// Reads `.tanren/asset-manifest`, compares against the embedded current
/// bundle, and returns a plan of actions and concerns. No files are written.
///
/// # Errors
///
/// Returns [`PreviewError`] if the root is inaccessible, the manifest is
/// missing or unparseable, or the manifest version is unsupported.
pub fn preview_upgrade(root: &Path) -> Result<UpgradePreviewResponse, PreviewError> {
    if !root.is_dir() {
        return Err(PreviewError::RootNotFound(root.to_path_buf()));
    }

    let manifest_path = root.join(".tanren").join("asset-manifest");
    if !manifest_path.is_file() {
        return Err(PreviewError::ManifestMissing(manifest_path));
    }

    let manifest = parse_manifest(&manifest_path)?;
    validate_manifest_version(manifest.version)?;

    let bundle = current_bundle();
    let installed_map: HashMap<&Path, &AssetEntry> = manifest
        .assets
        .iter()
        .map(|e| (e.path.as_path(), e))
        .collect();
    let bundle_map: HashMap<&Path, &BundledAsset> = bundle
        .assets
        .iter()
        .map(|a| (a.path.as_path(), a))
        .collect();

    let mut actions = Vec::new();
    let mut concerns = Vec::new();
    let mut preserved_user_paths = Vec::new();

    for bundled in &bundle.assets {
        if let Some(installed) = installed_map.get(bundled.path.as_path()) {
            if installed.ownership == AssetOwnership::User {
                actions.push(AssetAction::Preserve {
                    path: installed.path.clone(),
                    hash: installed.hash.clone(),
                });
                preserved_user_paths.push(installed.path.clone());
                continue;
            }
            if installed.hash != bundled.hash {
                concerns.push(MigrationConcern {
                    kind: MigrationConcernKind::HashMismatch,
                    path: installed.path.clone(),
                    detail: format!(
                        "Tanren asset was modified externally: {}",
                        installed.path.display()
                    ),
                });
                actions.push(AssetAction::Update {
                    path: installed.path.clone(),
                    old_hash: installed.hash.clone(),
                    new_hash: bundled.hash.clone(),
                });
            }
        } else {
            actions.push(AssetAction::Create {
                path: bundled.path.clone(),
                hash: bundled.hash.clone(),
            });
        }
    }

    for installed in &manifest.assets {
        if bundle_map.contains_key(installed.path.as_path()) {
            continue;
        }
        if installed.ownership == AssetOwnership::User {
            actions.push(AssetAction::Preserve {
                path: installed.path.clone(),
                hash: installed.hash.clone(),
            });
            preserved_user_paths.push(installed.path.clone());
            continue;
        }
        concerns.push(MigrationConcern {
            kind: MigrationConcernKind::RemovedAsset,
            path: installed.path.clone(),
            detail: format!(
                "Asset removed in target version: {}",
                installed.path.display()
            ),
        });
        actions.push(AssetAction::Remove {
            path: installed.path.clone(),
            old_hash: installed.hash.clone(),
        });
    }

    if manifest.version < MANIFEST_FORMAT_VERSION {
        concerns.push(MigrationConcern {
            kind: MigrationConcernKind::LegacyManifest,
            path: PathBuf::from(".tanren/asset-manifest"),
            detail: format!(
                "Manifest format version {} is older than current {}; verify after upgrade.",
                manifest.version, MANIFEST_FORMAT_VERSION
            ),
        });
    }

    Ok(UpgradePreviewResponse {
        source_version: manifest.source_version,
        target_version: bundle.version,
        actions,
        concerns,
        preserved_user_paths,
    })
}

fn parse_manifest(path: &Path) -> Result<AssetManifest, PreviewError> {
    let content =
        std::fs::read_to_string(path).map_err(|e| PreviewError::ManifestParse(e.to_string()))?;
    toml::from_str(&content).map_err(|e| PreviewError::ManifestParse(e.to_string()))
}

fn validate_manifest_version(version: u32) -> Result<(), PreviewError> {
    if version > MANIFEST_FORMAT_VERSION {
        return Err(PreviewError::UnsupportedVersion {
            manifest: version,
            supported: MANIFEST_FORMAT_VERSION,
        });
    }
    Ok(())
}
