//! Projection inspection — bounded drift checks between canonical
//! projection metadata and repo-local standards files.
//!
//! The [`ProjectionInspector`] compares a set of expected standards
//! entries (the *canonical projection*) against the files that actually
//! exist on disk inside a repo-local standards root. It enforces
//! structural bounds (file size, count, depth) *before* buffering any
//! file content, and rejects insecure paths (absolute roots, escaping
//! `..` components, symlink traversal) with typed failures.
//!
//! The root path is never accepted from CLI input directly — it comes
//! from the [`crate::standards::StandardsReadModel`] effective root.
//! Markdown contents on disk are compared against the canonical digest
//! but are never treated as the authoritative standards state.

use std::fmt::Write as _;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tanren_contract::StandardsFailureReason;
use thiserror::Error;

const DEFAULT_MAX_FILE_SIZE: u64 = 1_048_576;
const DEFAULT_MAX_FILE_COUNT: usize = 1000;
const DEFAULT_MAX_DEPTH: usize = 10;

/// Structural bounds enforced during projection inspection.
///
/// Bounds are checked *before* full-file buffering so pathological
/// trees are rejected early.
#[derive(Debug, Clone)]
pub struct ProjectionBounds {
    /// Maximum bytes per standards file.
    pub max_file_size: u64,
    /// Maximum number of standards files in the tree.
    pub max_file_count: usize,
    /// Maximum directory nesting depth for any entry.
    pub max_depth: usize,
}

impl Default for ProjectionBounds {
    fn default() -> Self {
        Self {
            max_file_size: DEFAULT_MAX_FILE_SIZE,
            max_file_count: DEFAULT_MAX_FILE_COUNT,
            max_depth: DEFAULT_MAX_DEPTH,
        }
    }
}

/// A single entry in the canonical projection manifest — the expected
/// state of a standards file as recorded when standards were installed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectionEntry {
    /// Relative path within the standards root.
    pub relative_path: PathBuf,
    /// Hex-encoded SHA-256 digest of the file content at install time.
    pub content_digest: String,
    /// File size in bytes at install time.
    pub size_bytes: u64,
}

/// The full manifest of expected standards entries against which the
/// repo-local filesystem is compared.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectionManifest {
    /// Expected standards entries.
    pub entries: Vec<ProjectionEntry>,
}

/// Status of a single standard's projection comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriftStatus {
    /// File content matches the canonical digest.
    Match,
    /// File does not exist on disk.
    Missing,
    /// File exists but content digest differs from canonical.
    Drifted {
        expected_digest: String,
        actual_digest: String,
    },
    /// File exists but is not a regular file (e.g. a directory).
    Malformed { reason: String },
}

/// Result of comparing a single standard against the projection.
#[derive(Debug, Clone)]
pub struct DriftItem {
    /// Relative path within the standards root.
    pub relative_path: PathBuf,
    /// Comparison outcome.
    pub status: DriftStatus,
}

/// Full projection inspection report.
#[derive(Debug, Clone)]
pub struct ProjectionReport {
    /// Per-entry drift results.
    pub items: Vec<DriftItem>,
    /// Total entries in the manifest.
    pub total_entries: usize,
    /// Entries that matched the canonical digest.
    pub matched: usize,
    /// Entries whose content has drifted.
    pub drifted: usize,
    /// Entries missing from disk.
    pub missing: usize,
    /// Entries that are malformed (not regular files).
    pub malformed: usize,
}

impl ProjectionReport {
    /// Whether every entry matches the canonical projection.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.drifted == 0 && self.missing == 0 && self.malformed == 0
    }
}

/// Typed failures raised during projection inspection.
///
/// Each variant identifies a specific security or structural violation
/// so callers can report the exact reason without relying on string
/// matching.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ProjectionError {
    /// The supplied root path is absolute — only repo-local relative
    /// roots are accepted.
    #[error("absolute root paths are not accepted: {path}")]
    AbsoluteRoot { path: PathBuf },
    /// A path component escapes the standards root with `..`.
    #[error("path escapes the standards root: {path}")]
    EscapingPath { path: PathBuf },
    /// A symlink was encountered during traversal.
    #[error("symlink traversal detected: {path}")]
    SymlinkTraversal { path: PathBuf },
    /// A file exceeds the configured size limit (checked before
    /// content buffering).
    #[error("file exceeds size limit ({size} > {limit}): {path}")]
    OversizedFile {
        path: PathBuf,
        size: u64,
        limit: u64,
    },
    /// The manifest contains more entries than the configured limit.
    #[error("file count exceeds limit ({count} > {limit})")]
    ExcessiveFileCount { count: usize, limit: usize },
    /// A manifest entry's path nesting exceeds the depth limit.
    #[error("traversal depth exceeds limit ({depth} > {limit})")]
    ExcessiveDepth { depth: usize, limit: usize },
    /// No standards root has been configured in the read model.
    #[error("standards root not configured")]
    NotConfigured,
    /// An I/O error occurred while reading a file or directory.
    #[error("I/O error: {0}")]
    Io(String),
}

impl From<ProjectionError> for StandardsFailureReason {
    fn from(value: ProjectionError) -> Self {
        match value {
            ProjectionError::AbsoluteRoot { .. }
            | ProjectionError::EscapingPath { .. }
            | ProjectionError::SymlinkTraversal { .. } => StandardsFailureReason::PathViolation,
            ProjectionError::OversizedFile { .. }
            | ProjectionError::ExcessiveFileCount { .. }
            | ProjectionError::ExcessiveDepth { .. } => StandardsFailureReason::TreeBoundsExceeded,
            ProjectionError::NotConfigured | ProjectionError::Io(_) => {
                StandardsFailureReason::StandardsRootNotFound
            }
        }
    }
}

/// Service that performs bounded projection inspection against a
/// repo-local standards root.
///
/// Construct with [`ProjectionInspector::new`] (custom bounds) or
/// [`ProjectionInspector::with_default_bounds`]. Call
/// [`ProjectionInspector::inspect`] with the relative root and the
/// canonical manifest.
#[derive(Debug, Clone)]
pub struct ProjectionInspector {
    bounds: ProjectionBounds,
}

impl Default for ProjectionInspector {
    fn default() -> Self {
        Self::with_default_bounds()
    }
}

impl ProjectionInspector {
    /// Construct with explicit bounds.
    #[must_use]
    pub fn new(bounds: ProjectionBounds) -> Self {
        Self { bounds }
    }

    /// Construct with the default bounding configuration.
    #[must_use]
    pub fn with_default_bounds() -> Self {
        Self::new(ProjectionBounds::default())
    }

    /// Borrow the active bounds.
    #[must_use]
    pub fn bounds(&self) -> &ProjectionBounds {
        &self.bounds
    }

    /// Compare repo-local files under `root` against the canonical
    /// `manifest`.
    ///
    /// The root must be a relative path (repo-local). Every manifest
    /// entry's relative path is validated for escaping components and
    /// depth before any I/O. File sizes are checked before content
    /// is buffered.
    ///
    /// # Errors
    ///
    /// Returns a [`ProjectionError`] variant for the first structural
    /// or security violation encountered.
    pub fn inspect(
        &self,
        root: &Path,
        manifest: &ProjectionManifest,
    ) -> Result<ProjectionReport, ProjectionError> {
        validate_root(root)?;

        if manifest.entries.len() > self.bounds.max_file_count {
            return Err(ProjectionError::ExcessiveFileCount {
                count: manifest.entries.len(),
                limit: self.bounds.max_file_count,
            });
        }

        let mut items = Vec::with_capacity(manifest.entries.len());
        let mut matched = 0;
        let mut drifted = 0;
        let mut missing = 0;
        let mut malformed = 0;

        for entry in &manifest.entries {
            let status = self.inspect_entry(root, entry)?;
            match &status {
                DriftStatus::Match => matched += 1,
                DriftStatus::Missing => missing += 1,
                DriftStatus::Drifted { .. } => drifted += 1,
                DriftStatus::Malformed { .. } => malformed += 1,
            }
            items.push(DriftItem {
                relative_path: entry.relative_path.clone(),
                status,
            });
        }

        Ok(ProjectionReport {
            total_entries: manifest.entries.len(),
            items,
            matched,
            drifted,
            missing,
            malformed,
        })
    }

    fn inspect_entry(
        &self,
        root: &Path,
        entry: &ProjectionEntry,
    ) -> Result<DriftStatus, ProjectionError> {
        validate_relative_path(&entry.relative_path)?;
        validate_depth(&entry.relative_path, self.bounds.max_depth)?;
        reject_symlinks_in_path(root, &entry.relative_path)?;

        let full_path = root.join(&entry.relative_path);

        let file_type = match std::fs::symlink_metadata(&full_path) {
            Ok(meta) => meta.file_type(),
            Err(_) => return Ok(DriftStatus::Missing),
        };

        if file_type.is_dir() {
            return Ok(DriftStatus::Malformed {
                reason: "expected a regular file, found a directory".to_owned(),
            });
        }

        let metadata =
            std::fs::metadata(&full_path).map_err(|e| ProjectionError::Io(e.to_string()))?;
        let size = metadata.len();

        if size > self.bounds.max_file_size {
            return Err(ProjectionError::OversizedFile {
                path: entry.relative_path.clone(),
                size,
                limit: self.bounds.max_file_size,
            });
        }

        let actual_digest = compute_digest(&full_path)?;

        if actual_digest == entry.content_digest {
            Ok(DriftStatus::Match)
        } else {
            Ok(DriftStatus::Drifted {
                expected_digest: entry.content_digest.clone(),
                actual_digest,
            })
        }
    }
}

fn reject_symlinks_in_path(root: &Path, relative: &Path) -> Result<(), ProjectionError> {
    match std::fs::symlink_metadata(root) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(ProjectionError::SymlinkTraversal {
                path: root.to_path_buf(),
            });
        }
        Err(_) => return Ok(()),
        _ => {}
    }

    let mut accumulated = PathBuf::from(root);
    for component in relative.components() {
        if let Component::Normal(seg) = component {
            accumulated.push(seg);
            match std::fs::symlink_metadata(&accumulated) {
                Ok(meta) if meta.file_type().is_symlink() => {
                    return Err(ProjectionError::SymlinkTraversal { path: accumulated });
                }
                Err(_) => break,
                _ => {}
            }
        }
    }

    Ok(())
}

fn validate_root(root: &Path) -> Result<(), ProjectionError> {
    if root.is_absolute() {
        return Err(ProjectionError::AbsoluteRoot {
            path: root.to_path_buf(),
        });
    }
    reject_escaping_components(root)
}

fn validate_relative_path(path: &Path) -> Result<(), ProjectionError> {
    reject_escaping_components(path)
}

fn reject_escaping_components(path: &Path) -> Result<(), ProjectionError> {
    for component in path.components() {
        match component {
            Component::ParentDir => {
                return Err(ProjectionError::EscapingPath {
                    path: path.to_path_buf(),
                });
            }
            Component::Prefix(_) | Component::RootDir => {
                return Err(ProjectionError::AbsoluteRoot {
                    path: path.to_path_buf(),
                });
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }
    Ok(())
}

fn validate_depth(path: &Path, max_depth: usize) -> Result<(), ProjectionError> {
    let depth = path
        .components()
        .filter(|c| matches!(c, Component::Normal(_)))
        .count();
    if depth > max_depth {
        return Err(ProjectionError::ExcessiveDepth {
            depth,
            limit: max_depth,
        });
    }
    Ok(())
}

fn compute_digest(path: &Path) -> Result<String, ProjectionError> {
    let data = std::fs::read(path).map_err(|e| ProjectionError::Io(e.to_string()))?;
    let hash = Sha256::digest(&data);
    let mut hex = String::with_capacity(hash.len() * 2);
    for byte in &hash {
        let _ = write!(hex, "{byte:02x}");
    }
    Ok(hex)
}
