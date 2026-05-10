//! Manifest-driven uninstall preview planning.

use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::install::catalog::{
    build_trusted_generated_asset_registry, is_trusted_generated_manifest_entry,
};
use crate::install::error::InstallError;
use crate::install::manifest::{
    INSTALL_MANIFEST_REPO_PATH, INSTALL_MANIFEST_VERSION, InstallManifest, ManifestEntry,
    PreservationPolicy, RepoRelativePath, Sha256Hex, sha256_hex,
};
use crate::install::path_guard::resolve_repo_path;

/// Why a manifest-tracked path is preserved during uninstall planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UninstallPreserveReason {
    /// Entry policy is not `replace-generated`.
    NotReplaceGenerated,
    /// Entry path is not part of Tanren's trusted generated-asset registry.
    UntrustedManifestEntry,
    /// Entry content no longer matches the manifest hash.
    ContentDrifted,
    /// Entry does not currently exist in the repository.
    MissingFromRepository,
    /// Entry path cannot be resolved safely inside the repository.
    UnsafeRepositoryPath,
}

/// Preserved path and the reason uninstall did not schedule removal.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UninstallPreservedPath {
    path: RepoRelativePath,
    reason: UninstallPreserveReason,
}

impl UninstallPreservedPath {
    /// Repo-relative path for the preserved entry.
    #[must_use]
    pub fn path(&self) -> &RepoRelativePath {
        &self.path
    }

    /// Preservation reason.
    #[must_use]
    pub const fn reason(&self) -> UninstallPreserveReason {
        self.reason
    }
}

/// Warning category emitted by uninstall planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UninstallWarningKind {
    /// No install manifest exists, so uninstall has nothing to plan.
    ManifestMissing,
    /// Manifest entry is not trusted as a Tanren-generated asset.
    UntrustedManifestEntry,
    /// Manifest entry path cannot be resolved safely.
    UnsafeRepositoryPath,
    /// Manifest-tracked generated content drifted and is preserved.
    ContentDrifted,
}

/// Warning emitted during uninstall planning.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UninstallWarning {
    kind: UninstallWarningKind,
    path: Option<RepoRelativePath>,
}

impl UninstallWarning {
    /// Warning category.
    #[must_use]
    pub const fn kind(&self) -> UninstallWarningKind {
        self.kind
    }

    /// Optional repo-relative path associated with the warning.
    #[must_use]
    pub fn path(&self) -> Option<&RepoRelativePath> {
        self.path.as_ref()
    }
}

/// Why uninstall preview has nothing scheduled for removal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UninstallNothingReason {
    /// No prior install manifest exists in the repository.
    ManifestMissing,
    /// Manifest entries were present, but none qualified for removal.
    NoRemovalCandidates,
}

/// Non-mutating uninstall preview grouped into deterministic outcome buckets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallPreview {
    remove: Vec<RepoRelativePath>,
    preserve: Vec<UninstallPreservedPath>,
    warning: Vec<UninstallWarning>,
    nothing_to_uninstall: bool,
    nothing_reason: Option<UninstallNothingReason>,
}

impl UninstallPreview {
    /// Repo-relative paths scheduled for removal.
    #[must_use]
    pub fn remove(&self) -> &[RepoRelativePath] {
        &self.remove
    }

    /// Preserved paths and their reasons.
    #[must_use]
    pub fn preserve(&self) -> &[UninstallPreservedPath] {
        &self.preserve
    }

    /// Warning entries emitted while planning.
    #[must_use]
    pub fn warning(&self) -> &[UninstallWarning] {
        &self.warning
    }

    /// Whether there are zero removal candidates.
    #[must_use]
    pub const fn nothing_to_uninstall(&self) -> bool {
        self.nothing_to_uninstall
    }

    /// Typed no-op reason when no uninstall removals are scheduled.
    #[must_use]
    pub const fn nothing_reason(&self) -> Option<UninstallNothingReason> {
        self.nothing_reason
    }
}

pub(super) fn build_uninstall_preview(repository: &Path) -> Result<UninstallPreview, InstallError> {
    let repository_root = validate_repository_root(repository)?;
    let manifest_absolute_path = resolve_manifest_path(&repository_root)?;
    if !manifest_absolute_path.exists() {
        return Ok(manifest_missing_preview());
    }

    let manifest = load_manifest(&manifest_absolute_path)?;
    validate_manifest_version(&manifest)?;
    let trusted_generated_assets = build_trusted_generated_asset_registry()?;
    let mut outcomes = PreviewOutcomes::default();
    let mut seen_paths = BTreeSet::new();

    for entry in &manifest.entries {
        plan_manifest_entry(
            entry,
            &repository_root,
            trusted_generated_assets,
            &mut seen_paths,
            &mut outcomes,
        )?;
    }

    Ok(outcomes.finish())
}

fn resolve_manifest_path(repository_root: &Path) -> Result<PathBuf, InstallError> {
    let manifest_path = RepoRelativePath::parse(INSTALL_MANIFEST_REPO_PATH)?;
    resolve_repo_path(repository_root, &manifest_path)
}

fn manifest_missing_preview() -> UninstallPreview {
    UninstallPreview {
        remove: Vec::new(),
        preserve: Vec::new(),
        warning: vec![UninstallWarning {
            kind: UninstallWarningKind::ManifestMissing,
            path: None,
        }],
        nothing_to_uninstall: true,
        nothing_reason: Some(UninstallNothingReason::ManifestMissing),
    }
}

#[derive(Debug, Default)]
struct PreviewOutcomes {
    remove: Vec<RepoRelativePath>,
    preserve: Vec<UninstallPreservedPath>,
    warning: Vec<UninstallWarning>,
}

impl PreviewOutcomes {
    fn preserve(&mut self, path: RepoRelativePath, reason: UninstallPreserveReason) {
        self.preserve.push(UninstallPreservedPath { path, reason });
    }

    fn warn(&mut self, kind: UninstallWarningKind, path: RepoRelativePath) {
        self.warning.push(UninstallWarning {
            kind,
            path: Some(path),
        });
    }

    fn remove(&mut self, path: RepoRelativePath) {
        self.remove.push(path);
    }

    fn finish(mut self) -> UninstallPreview {
        self.remove.sort();
        self.remove.dedup();
        self.preserve.sort();
        self.preserve.dedup();
        self.warning.sort();
        self.warning.dedup();
        let nothing_to_uninstall = self.remove.is_empty();
        UninstallPreview {
            nothing_reason: if nothing_to_uninstall {
                Some(UninstallNothingReason::NoRemovalCandidates)
            } else {
                None
            },
            nothing_to_uninstall,
            remove: self.remove,
            preserve: self.preserve,
            warning: self.warning,
        }
    }
}

fn plan_manifest_entry<'a>(
    entry: &'a ManifestEntry,
    repository_root: &Path,
    trusted_generated_assets: &BTreeSet<RepoRelativePath>,
    seen_paths: &mut BTreeSet<&'a str>,
    outcomes: &mut PreviewOutcomes,
) -> Result<(), InstallError> {
    let path_key = entry.path.as_str();
    if !seen_paths.insert(path_key) {
        return Err(InstallError::InvalidInstallManifest {
            path: INSTALL_MANIFEST_REPO_PATH.to_owned(),
            message: format!("duplicate manifest entry for '{path_key}'"),
        });
    }

    if entry.preservation != PreservationPolicy::ReplaceGenerated {
        outcomes.preserve(
            entry.path.clone(),
            UninstallPreserveReason::NotReplaceGenerated,
        );
        return Ok(());
    }

    if !trusted_generated_assets.contains(&entry.path)
        || !is_trusted_generated_manifest_entry(entry)
    {
        outcomes.preserve(
            entry.path.clone(),
            UninstallPreserveReason::UntrustedManifestEntry,
        );
        outcomes.warn(
            UninstallWarningKind::UntrustedManifestEntry,
            entry.path.clone(),
        );
        return Ok(());
    }

    let Some(absolute_path) = resolve_uninstall_target(repository_root, entry)? else {
        outcomes.preserve(
            entry.path.clone(),
            UninstallPreserveReason::UnsafeRepositoryPath,
        );
        outcomes.warn(
            UninstallWarningKind::UnsafeRepositoryPath,
            entry.path.clone(),
        );
        return Ok(());
    };

    if !absolute_path.exists() {
        outcomes.preserve(
            entry.path.clone(),
            UninstallPreserveReason::MissingFromRepository,
        );
        return Ok(());
    }

    match classify_removal_candidate(&absolute_path, entry.path.as_str(), &entry.content_hash)? {
        RemovalCandidateState::Remove => outcomes.remove(entry.path.clone()),
        RemovalCandidateState::PreserveAsContentDrifted => {
            outcomes.preserve(entry.path.clone(), UninstallPreserveReason::ContentDrifted);
            outcomes.warn(UninstallWarningKind::ContentDrifted, entry.path.clone());
        }
        RemovalCandidateState::PreserveAsUnsafeRepositoryPath => {
            outcomes.preserve(
                entry.path.clone(),
                UninstallPreserveReason::UnsafeRepositoryPath,
            );
            outcomes.warn(
                UninstallWarningKind::UnsafeRepositoryPath,
                entry.path.clone(),
            );
        }
    }
    Ok(())
}

fn resolve_uninstall_target(
    repository_root: &Path,
    entry: &ManifestEntry,
) -> Result<Option<PathBuf>, InstallError> {
    match resolve_repo_path(repository_root, &entry.path) {
        Ok(path) => Ok(Some(path)),
        Err(
            InstallError::UnsafeRepositoryPath { .. }
            | InstallError::InvalidRepoRelativePath { .. },
        ) => Ok(None),
        Err(error) => Err(error),
    }
}

fn validate_repository_root(repository: &Path) -> Result<PathBuf, InstallError> {
    let canonical =
        repository
            .canonicalize()
            .map_err(|err| InstallError::InvalidRepositoryPath {
                path: format!(
                    "{} ({})",
                    display_repository_argument(repository),
                    redacted_io_error_kind(err.kind())
                ),
            })?;

    if !canonical.is_dir() {
        return Err(InstallError::RepositoryPathNotDirectory {
            path: display_repository_argument(repository),
        });
    }

    Ok(canonical)
}

fn load_manifest(manifest_path: &Path) -> Result<InstallManifest, InstallError> {
    let raw_manifest =
        fs::read_to_string(manifest_path).map_err(|err| InstallError::InvalidInstallManifest {
            path: INSTALL_MANIFEST_REPO_PATH.to_owned(),
            message: format!("failed to read manifest: {err}"),
        })?;

    toml::from_str(&raw_manifest).map_err(|err| InstallError::InvalidInstallManifest {
        path: INSTALL_MANIFEST_REPO_PATH.to_owned(),
        message: err.to_string(),
    })
}

fn validate_manifest_version(manifest: &InstallManifest) -> Result<(), InstallError> {
    if manifest.manifest_version != INSTALL_MANIFEST_VERSION {
        return Err(InstallError::InvalidInstallManifest {
            path: INSTALL_MANIFEST_REPO_PATH.to_owned(),
            message: format!("unsupported manifest version {}", manifest.manifest_version),
        });
    }
    Ok(())
}

fn hash_matches_manifest(
    path: &Path,
    display_path: &str,
    expected_hash: &Sha256Hex,
) -> Result<bool, InstallError> {
    let mut file = fs::File::open(path).map_err(|err| InstallError::ReadFailure {
        path: display_path.to_owned(),
        message: err.to_string(),
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 16_384];
    loop {
        let read_count = file
            .read(&mut buffer)
            .map_err(|err| InstallError::ReadFailure {
                path: display_path.to_owned(),
                message: err.to_string(),
            })?;
        if read_count == 0 {
            break;
        }
        hasher.update(&buffer[..read_count]);
    }
    let observed_hash = sha256_hex(&hasher.finalize());
    Ok(observed_hash == *expected_hash)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RemovalCandidateState {
    Remove,
    PreserveAsContentDrifted,
    PreserveAsUnsafeRepositoryPath,
}

fn classify_removal_candidate(
    path: &Path,
    display_path: &str,
    expected_hash: &Sha256Hex,
) -> Result<RemovalCandidateState, InstallError> {
    let metadata = fs::symlink_metadata(path).map_err(|err| InstallError::ReadFailure {
        path: display_path.to_owned(),
        message: err.to_string(),
    })?;
    if metadata.file_type().is_symlink() {
        return Ok(RemovalCandidateState::PreserveAsUnsafeRepositoryPath);
    }
    if !metadata.is_file() {
        return Ok(RemovalCandidateState::PreserveAsContentDrifted);
    }

    if hash_matches_manifest(path, display_path, expected_hash)? {
        return Ok(RemovalCandidateState::Remove);
    }

    Ok(RemovalCandidateState::PreserveAsContentDrifted)
}

fn display_repository_argument(path: &Path) -> String {
    if path.is_absolute() {
        "<redacted-absolute-path>".to_owned()
    } else {
        path.display().to_string()
    }
}

fn redacted_io_error_kind(kind: std::io::ErrorKind) -> &'static str {
    match kind {
        std::io::ErrorKind::NotFound => "not_found",
        std::io::ErrorKind::PermissionDenied => "permission_denied",
        std::io::ErrorKind::AlreadyExists => "already_exists",
        std::io::ErrorKind::InvalidInput => "invalid_input",
        std::io::ErrorKind::InvalidData => "invalid_data",
        std::io::ErrorKind::TimedOut => "timed_out",
        std::io::ErrorKind::WriteZero => "write_zero",
        std::io::ErrorKind::Interrupted => "interrupted",
        std::io::ErrorKind::Unsupported => "unsupported",
        std::io::ErrorKind::UnexpectedEof => "unexpected_eof",
        _ => "io_error",
    }
}
