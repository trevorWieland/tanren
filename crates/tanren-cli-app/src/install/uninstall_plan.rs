//! Manifest-driven uninstall preview planning.

use std::collections::BTreeSet;
use std::fmt;
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
use crate::install::path_guard::{resolve_repo_path, validate_repository_root};

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
    /// Entry content hash is structurally invalid (wrong length or non-hex characters).
    MalformedHash,
}

impl fmt::Display for UninstallPreserveReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotReplaceGenerated => f.write_str("not_replace_generated"),
            Self::UntrustedManifestEntry => f.write_str("untrusted_manifest_entry"),
            Self::ContentDrifted => f.write_str("content_drifted"),
            Self::MissingFromRepository => f.write_str("missing_from_repository"),
            Self::UnsafeRepositoryPath => f.write_str("unsafe_repository_path"),
            Self::MalformedHash => f.write_str("malformed_hash"),
        }
    }
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
    /// Manifest contains duplicate entry paths.
    DuplicateManifestEntry,
}

impl fmt::Display for UninstallWarningKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManifestMissing => f.write_str("manifest_missing"),
            Self::UntrustedManifestEntry => f.write_str("untrusted_manifest_entry"),
            Self::UnsafeRepositoryPath => f.write_str("unsafe_repository_path"),
            Self::ContentDrifted => f.write_str("content_drifted"),
            Self::DuplicateManifestEntry => f.write_str("duplicate_manifest_entry"),
        }
    }
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

impl fmt::Display for UninstallNothingReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManifestMissing => f.write_str("manifest_missing"),
            Self::NoRemovalCandidates => f.write_str("no_removal_candidates"),
        }
    }
}

/// Non-mutating uninstall preview grouped into deterministic outcome buckets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallPreview {
    remove: Vec<RepoRelativePath>,
    preserve: Vec<UninstallPreservedPath>,
    warning: Vec<UninstallWarning>,
    nothing_to_uninstall: bool,
    nothing_reason: Option<UninstallNothingReason>,
    repository_root: PathBuf,
    manifest_fingerprint: Option<Sha256Hex>,
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

    /// Total classified entries (remove + preserve + warning) for allocation hints.
    #[must_use]
    pub fn total_entry_count(&self) -> usize {
        self.remove.len() + self.preserve.len() + self.warning.len()
    }

    /// Canonical repository root observed at planning time.
    #[must_use]
    pub fn repository_root(&self) -> &Path {
        &self.repository_root
    }

    /// SHA-256 fingerprint of the manifest content at planning time.
    #[must_use]
    pub fn manifest_fingerprint(&self) -> Option<&Sha256Hex> {
        self.manifest_fingerprint.as_ref()
    }
}

pub(super) fn build_uninstall_preview(repository: &Path) -> Result<UninstallPreview, InstallError> {
    let repository_root = validate_repository_root(repository)?;
    let manifest_absolute_path = resolve_manifest_path(&repository_root)?;
    if !manifest_absolute_path.exists() {
        return Ok(manifest_missing_preview(repository_root));
    }

    let manifest_bytes =
        fs::read(&manifest_absolute_path).map_err(|err| InstallError::ReadFailure {
            path: INSTALL_MANIFEST_REPO_PATH.to_owned(),
            message: err.to_string(),
        })?;
    let manifest_fingerprint = sha256_hex(&manifest_bytes);

    let manifest = parse_manifest_bytes(&manifest_bytes)?;
    validate_manifest_version(&manifest)?;
    let trusted_generated_assets = build_trusted_generated_asset_registry()?;
    let entry_count = manifest.entries.len();
    let mut outcomes = PreviewOutcomes::with_capacity(entry_count);
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

    Ok(outcomes.finish(repository_root, Some(manifest_fingerprint)))
}

fn resolve_manifest_path(repository_root: &Path) -> Result<PathBuf, InstallError> {
    let manifest_path = RepoRelativePath::parse(INSTALL_MANIFEST_REPO_PATH)?;
    resolve_repo_path(repository_root, &manifest_path)
}

fn manifest_missing_preview(repository_root: PathBuf) -> UninstallPreview {
    UninstallPreview {
        remove: Vec::new(),
        preserve: Vec::new(),
        warning: vec![UninstallWarning {
            kind: UninstallWarningKind::ManifestMissing,
            path: None,
        }],
        nothing_to_uninstall: true,
        nothing_reason: Some(UninstallNothingReason::ManifestMissing),
        repository_root,
        manifest_fingerprint: None,
    }
}

fn parse_manifest_bytes(bytes: &[u8]) -> Result<InstallManifest, InstallError> {
    let raw =
        String::from_utf8(bytes.to_vec()).map_err(|err| InstallError::InvalidInstallManifest {
            path: INSTALL_MANIFEST_REPO_PATH.to_owned(),
            message: err.to_string(),
        })?;
    toml::from_str(&raw).map_err(|err| InstallError::InvalidInstallManifest {
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

    fn with_capacity(capacity: usize) -> Self {
        Self {
            remove: Vec::with_capacity(capacity),
            preserve: Vec::with_capacity(capacity),
            warning: Vec::with_capacity(capacity),
        }
    }

    fn remove(&mut self, path: RepoRelativePath) {
        self.remove.push(path);
    }

    fn finish(
        mut self,
        repository_root: PathBuf,
        manifest_fingerprint: Option<Sha256Hex>,
    ) -> UninstallPreview {
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
            repository_root,
            manifest_fingerprint,
        }
    }
}

fn plan_manifest_entry<'a>(
    entry: &'a ManifestEntry,
    repository_root: &Path,
    _trusted_generated_assets: &BTreeSet<RepoRelativePath>,
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

    if !is_trusted_generated_manifest_entry(entry) {
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

    let target = resolve_uninstall_target(repository_root, entry)?;
    let Some(target_path) = target else {
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

    if !target_path.exists() {
        outcomes.preserve(
            entry.path.clone(),
            UninstallPreserveReason::MissingFromRepository,
        );
        return Ok(());
    }

    if !validate_content_hash(&entry.content_hash) {
        outcomes.preserve(entry.path.clone(), UninstallPreserveReason::MalformedHash);
        return Ok(());
    }

    let state = classify_removal_candidate(&target_path, entry.path.as_str(), &entry.content_hash)?;
    match state {
        RemovalCandidateState::Remove => {
            outcomes.remove(entry.path.clone());
        }
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

fn validate_content_hash(hash: &Sha256Hex) -> bool {
    Sha256Hex::parse(hash.as_str()).is_ok()
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
