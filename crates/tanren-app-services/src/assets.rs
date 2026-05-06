//! Asset upgrade preview planner and confirmed apply executor.
//!
//! Reads the installed `.tanren/asset-manifest` from a repository root,
//! compares it against the embedded current asset bundle, and produces an
//! [`UpgradePreviewResponse`] describing what would change. The preview is
//! read-only — no files are written.
//!
//! When confirmed, [`apply_upgrade`] executes the planned actions: it creates,
//! updates, and removes Tanren-owned assets while preserving user-owned files,
//! then rewrites the manifest to the new version.

use std::collections::HashMap;
use std::fmt::Write;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
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

/// Error taxonomy for the confirmed asset upgrade apply flow.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ApplyError {
    /// A preview error prevented planning the upgrade.
    #[error("upgrade preview failed: {0}")]
    Preview(#[from] PreviewError),
    /// A generated asset was modified on disk since the manifest was written
    /// and no corresponding concern was flagged during preview.
    #[error(
        "drift detected for {path}: on-disk hash {observed} differs from manifest hash {recorded}, and the preview did not report this conflict"
    )]
    UnreportedDrift {
        path: PathBuf,
        recorded: String,
        observed: String,
    },
    /// An I/O error occurred while reading or writing an asset.
    #[error("I/O error for {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// The manifest could not be rewritten after applying changes.
    #[error("failed to rewrite manifest: {0}")]
    ManifestWrite(String),
}

/// A single asset in the embedded current bundle.
#[derive(Debug, Clone)]
pub struct BundledAsset {
    /// Relative path from the repository root.
    pub path: PathBuf,
    /// Content hash in the form `sha256:<hex>`.
    pub hash: String,
    /// File content bytes to write on create or update.
    pub content: Vec<u8>,
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
    let config_content = b"# Tanren configuration\n";
    let check_content = b"# Check command documentation\n";
    let build_content = b"# Build command documentation\n";

    AssetBundle {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        assets: vec![
            BundledAsset {
                path: PathBuf::from(".tanren/config.toml"),
                hash: compute_hash(config_content),
                content: config_content.to_vec(),
            },
            BundledAsset {
                path: PathBuf::from("commands/check.md"),
                hash: compute_hash(check_content),
                content: check_content.to_vec(),
            },
            BundledAsset {
                path: PathBuf::from("commands/build.md"),
                hash: compute_hash(build_content),
                content: build_content.to_vec(),
            },
        ],
    }
}

fn compute_hash(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    let result = hasher.finalize();
    let mut hex = String::with_capacity(result.len() * 2);
    for byte in result {
        let _ = write!(hex, "{byte:02x}");
    }
    format!("sha256:{hex}")
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

/// Execute a confirmed upgrade: apply all planned actions from the preview,
/// protect user-owned files, and rewrite the manifest to the new version.
///
/// Internally calls [`preview_upgrade`] to compute the plan, then applies
/// each action. User-owned files (`Preserve` actions) are never touched.
/// For `Update` actions, the on-disk hash is verified against the manifest
/// record; if it differs and the preview did not flag a `HashMismatch`
/// concern, the apply is aborted with [`ApplyError::UnreportedDrift`].
///
/// # Errors
///
/// Returns [`ApplyError`] for preview failures, unreported drift, or I/O
/// errors during file writes.
pub fn apply_upgrade(root: &Path) -> Result<UpgradePreviewResponse, ApplyError> {
    let preview = preview_upgrade(root)?;

    let manifest_path = root.join(".tanren").join("asset-manifest");
    let manifest = parse_manifest(&manifest_path)?;
    let bundle = current_bundle();

    let bundle_content: HashMap<&Path, &[u8]> = bundle
        .assets
        .iter()
        .map(|a| (a.path.as_path(), a.content.as_slice()))
        .collect();

    let _installed_map: HashMap<&Path, &AssetEntry> = manifest
        .assets
        .iter()
        .map(|e| (e.path.as_path(), e))
        .collect();

    let concern_paths: HashMap<&Path, MigrationConcernKind> = preview
        .concerns
        .iter()
        .map(|c| (c.path.as_path(), c.kind))
        .collect();

    let mut new_entries: Vec<AssetEntry> = manifest.assets.clone();

    for action in &preview.actions {
        match action {
            AssetAction::Create { path, hash: _ } => {
                let full_path = root.join(path);
                if let Some(parent) = full_path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| ApplyError::Io {
                        path: parent.to_path_buf(),
                        source: e,
                    })?;
                }
                let content = bundle_content.get(path.as_path()).copied().unwrap_or(&[]);
                std::fs::write(&full_path, content).map_err(|e| ApplyError::Io {
                    path: full_path.clone(),
                    source: e,
                })?;
                new_entries.push(AssetEntry {
                    path: path.clone(),
                    hash: compute_hash(content),
                    ownership: AssetOwnership::Tanren,
                    installed_from: bundle.version.clone(),
                });
            }
            AssetAction::Update {
                path,
                old_hash,
                new_hash: _,
            } => {
                let full_path = root.join(path);
                if let Some(observed) = read_file_hash(&full_path) {
                    if &observed != old_hash {
                        let is_reported = concern_paths
                            .get(path.as_path())
                            .is_some_and(|k| *k == MigrationConcernKind::HashMismatch);
                        if !is_reported {
                            return Err(ApplyError::UnreportedDrift {
                                path: path.clone(),
                                recorded: old_hash.clone(),
                                observed,
                            });
                        }
                    }
                }
                if let Some(parent) = full_path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| ApplyError::Io {
                        path: parent.to_path_buf(),
                        source: e,
                    })?;
                }
                let content = bundle_content.get(path.as_path()).copied().unwrap_or(&[]);
                std::fs::write(&full_path, content).map_err(|e| ApplyError::Io {
                    path: full_path.clone(),
                    source: e,
                })?;
                if let Some(entry) = new_entries.iter_mut().find(|e| e.path == *path) {
                    entry.hash = compute_hash(content);
                    entry.installed_from.clone_from(&bundle.version);
                }
            }
            AssetAction::Remove { path, old_hash: _ } => {
                let full_path = root.join(path);
                if full_path.is_file() {
                    std::fs::remove_file(&full_path).map_err(|e| ApplyError::Io {
                        path: full_path.clone(),
                        source: e,
                    })?;
                }
                new_entries.retain(|e| e.path != *path);
            }
            AssetAction::Preserve { .. } => {}
        }
    }

    let new_manifest = AssetManifest {
        version: MANIFEST_FORMAT_VERSION,
        source_version: bundle.version,
        assets: new_entries,
    };
    write_manifest(&manifest_path, &new_manifest)?;

    Ok(preview)
}

fn read_file_hash(path: &Path) -> Option<String> {
    let content = std::fs::read(path).ok()?;
    Some(compute_hash(&content))
}

fn write_manifest(path: &Path, manifest: &AssetManifest) -> Result<(), ApplyError> {
    let content =
        toml::to_string_pretty(manifest).map_err(|e| ApplyError::ManifestWrite(e.to_string()))?;
    std::fs::write(path, content).map_err(|e| ApplyError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    Ok(())
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
