//! Project-uninstall handlers: preview and apply.
//!
//! Preview reads the install manifest from a repository and classifies
//! every tracked file as either safe-to-remove (unchanged
//! Tanren-generated) or preserved (user-owned, modified, or already
//! absent). Apply removes only the safe-to-remove files and the
//! manifest itself. Neither function mutates the filesystem until
//! [`apply`] is called.

use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};
use tanren_contract::project_uninstall::{
    FileOwnership, InstallManifest, MANIFEST_V1_PATH, PreserveReason, PreservedFile,
    UninstallPreview, UninstallResult,
};
use thiserror::Error;

/// Errors raised by project-uninstall handlers.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum UninstallError {
    /// No install manifest was found in the repository.
    #[error("no install manifest found at {path}")]
    NoManifest {
        /// Path where the manifest was expected.
        path: String,
    },
    /// The manifest file could not be read.
    #[error("failed to read manifest at {path}: {source}")]
    ReadFailed {
        /// Path to the manifest.
        path: String,
        /// Underlying I/O error.
        source: std::io::Error,
    },
    /// The manifest file could not be parsed.
    #[error("failed to parse install manifest: {source}")]
    ParseFailed {
        /// Underlying parse error.
        source: serde_json::Error,
    },
    /// A filesystem operation failed during apply.
    #[error("I/O error during uninstall: {0}")]
    Io(#[from] std::io::Error),
}

/// Read the install manifest from a repository.
///
/// # Errors
///
/// Returns [`UninstallError::NoManifest`] if the manifest file is
/// absent, [`UninstallError::ReadFailed`] on I/O failure, or
/// [`UninstallError::ParseFailed`] on invalid JSON.
pub fn read_manifest(repo: &Path) -> Result<InstallManifest, UninstallError> {
    let manifest_path = repo.join(MANIFEST_V1_PATH);
    let manifest_str = manifest_path.display().to_string();

    if !manifest_path.exists() {
        return Err(UninstallError::NoManifest { path: manifest_str });
    }

    let content = fs::read_to_string(&manifest_path).map_err(|e| UninstallError::ReadFailed {
        path: manifest_str,
        source: e,
    })?;

    serde_json::from_str(&content).map_err(|e| UninstallError::ParseFailed { source: e })
}

/// Compute a preview of what uninstall would do, without modifying
/// anything on disk.
///
/// # Errors
///
/// Returns [`UninstallError`] if the manifest cannot be read or
/// parsed.
pub fn preview(repo: &Path) -> Result<UninstallPreview, UninstallError> {
    classify_entries(repo)
}

/// Apply the uninstall: remove unchanged Tanren-generated files and
/// the manifest itself. User-owned and modified files are preserved.
///
/// # Errors
///
/// Returns [`UninstallError`] if the manifest cannot be read or if a
/// filesystem operation fails.
pub fn apply(repo: &Path) -> Result<UninstallResult, UninstallError> {
    let preview_result = preview(repo)?;

    let mut removed: Vec<String> = Vec::new();
    for rel_path in &preview_result.to_remove {
        let full = repo.join(rel_path);
        if full.exists() {
            fs::remove_file(&full)?;
            removed.push(rel_path.clone());
        }
    }

    let manifest_full = repo.join(&preview_result.manifest_path);
    let manifest_removed = if manifest_full.exists() {
        fs::remove_file(&manifest_full)?;
        clean_empty_parents(&manifest_full);
        true
    } else {
        false
    };

    Ok(UninstallResult {
        removed,
        preserved: preview_result.preserved,
        manifest_removed,
    })
}

/// Compute the SHA-256 hex digest of a file's contents.
fn file_hash(path: &Path) -> Result<String, std::io::Error> {
    let bytes = fs::read(path)?;
    let digest = Sha256::digest(&bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest.as_slice() {
        use std::fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    Ok(hex)
}

/// Classify every entry in the manifest as to-remove or preserved.
fn classify_entries(repo: &Path) -> Result<UninstallPreview, UninstallError> {
    let manifest = read_manifest(repo)?;
    let mut to_remove: Vec<String> = Vec::new();
    let mut preserved: Vec<PreservedFile> = Vec::new();

    for entry in &manifest.entries {
        let full = repo.join(&entry.path);

        match entry.ownership {
            FileOwnership::UserOwned => {
                preserved.push(PreservedFile {
                    path: entry.path.clone(),
                    reason: PreserveReason::UserOwned,
                });
            }
            FileOwnership::TanrenGenerated => {
                if full.exists() {
                    match file_hash(&full) {
                        Ok(hash) if hash == entry.content_hash => {
                            to_remove.push(entry.path.clone());
                        }
                        _ => {
                            preserved.push(PreservedFile {
                                path: entry.path.clone(),
                                reason: PreserveReason::ModifiedSinceInstall,
                            });
                        }
                    }
                } else {
                    preserved.push(PreservedFile {
                        path: entry.path.clone(),
                        reason: PreserveReason::AlreadyRemoved,
                    });
                }
            }
        }
    }

    Ok(UninstallPreview {
        to_remove,
        preserved,
        manifest_path: MANIFEST_V1_PATH.to_owned(),
    })
}

/// Remove empty parent directories left behind after file removal.
fn clean_empty_parents(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = fs::remove_dir(parent);
    }
}
