//! Filesystem writer for manifest-driven install plans.

use std::io::ErrorKind;
use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::install::error::InstallError;
use crate::install::manifest::{
    INSTALL_MANIFEST_REPO_PATH, InstallManifest, RepoRelativePath, Sha256Hex, sha256_hex,
};
use crate::install::path_guard::{resolve_repo_path, validate_repository_root};
use crate::install::plan::{InstallPlan, PlannedWriteKind};
use crate::install::uninstall_plan::UninstallPreview;
use crate::install::writer_tx::{
    atomic_replace_file, cleanup_staged_payloads, commit_staged_replacement, prepare_apply,
    resolve_apply_failure,
};

/// Install apply report grouped by observable outcome.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstallReport {
    pub created: Vec<RepoRelativePath>,
    pub updated: Vec<RepoRelativePath>,
    pub removed: Vec<RepoRelativePath>,
    pub restored: Vec<RepoRelativePath>,
    pub preserved: Vec<RepoRelativePath>,
}

impl InstallReport {
    fn sort_paths(&mut self) {
        self.created.sort();
        self.updated.sort();
        self.removed.sort();
        self.restored.sort();
        self.preserved.sort();
    }
}

/// Uninstall apply report grouped by generated assets vs install metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UninstallApplyReport {
    pub removed_generated: Vec<RepoRelativePath>,
    pub removed_metadata: Vec<RepoRelativePath>,
    pub preserved_on_drift: Vec<RepoRelativePath>,
}

impl UninstallApplyReport {
    fn sort_paths(&mut self) {
        self.removed_generated.sort();
        self.removed_metadata.sort();
        self.preserved_on_drift.sort();
    }
}

#[derive(Debug, Clone)]
struct PreparedUninstallRemoval {
    path: RepoRelativePath,
    absolute: PathBuf,
    prior_content: Vec<u8>,
    class: UninstallRemovalClass,
    expected_hash: Option<Sha256Hex>,
}

#[derive(Debug, Clone)]
struct PlannedUninstallRemoval {
    path: RepoRelativePath,
    class: UninstallRemovalClass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UninstallRemovalClass {
    GeneratedAsset,
    InstallMetadata,
}

/// Apply a previously validated install plan.
pub(super) fn apply_install_plan(plan: &InstallPlan) -> Result<InstallReport, InstallError> {
    let prepared = prepare_apply(plan)?;
    let mut report = InstallReport {
        preserved: plan.preserved().to_vec(),
        ..InstallReport::default()
    };
    let mut changed_paths = Vec::new();

    let apply_result: Result<(), InstallError> = (|| {
        for removal in &prepared.removals {
            std::fs::remove_file(&removal.absolute).map_err(|err| InstallError::RemoveFailure {
                path: removal.path.as_str().to_owned(),
                message: err.to_string(),
            })?;
            report.removed.push(removal.path.clone());
            changed_paths.push(removal.path.clone());
        }

        for write in &prepared.writes {
            commit_staged_replacement(plan, &write.staged)?;
            match write.kind {
                PlannedWriteKind::Created => report.created.push(write.staged.path.clone()),
                PlannedWriteKind::Updated => report.updated.push(write.staged.path.clone()),
                PlannedWriteKind::Restored => report.restored.push(write.staged.path.clone()),
            }
            changed_paths.push(write.staged.path.clone());
        }

        commit_staged_replacement(plan, &prepared.manifest.staged)?;
        if prepared.manifest.prior.is_none() {
            report.created.push(prepared.manifest.staged.path.clone());
        } else if prepared.manifest.prior.as_deref() != Some(prepared.manifest.payload.as_slice()) {
            report.updated.push(prepared.manifest.staged.path.clone());
        }
        changed_paths.push(prepared.manifest.staged.path.clone());

        Ok(())
    })();

    cleanup_staged_payloads(&prepared);

    if let Err(error) = apply_result {
        return Err(resolve_apply_failure(
            plan,
            &prepared.rollback_records,
            &changed_paths,
            error,
        ));
    }

    report.sort_paths();
    Ok(report)
}

/// Apply a previously emitted uninstall preview.
pub(super) fn apply_uninstall_preview(
    repository: &Path,
    preview: &UninstallPreview,
) -> Result<UninstallApplyReport, InstallError> {
    let repository_root = validate_repository_root(repository)?;
    validate_preview_root(&repository_root, preview)?;
    validate_preview_manifest_fingerprint(&repository_root, preview)?;

    let manifest_path = RepoRelativePath::parse(INSTALL_MANIFEST_REPO_PATH)?;
    let manifest_absolute = resolve_repo_path(&repository_root, &manifest_path)?;
    let manifest_entries = load_manifest_hashes(&manifest_absolute)?;

    let planned_removals =
        build_uninstall_removal_plan(preview.remove(), manifest_absolute.exists(), &manifest_path);
    if planned_removals.is_empty() {
        return Ok(UninstallApplyReport::default());
    }

    let prepared = prepare_uninstall_apply(&repository_root, &planned_removals, &manifest_entries)?;
    let mut report = UninstallApplyReport::default();
    let mut changed_indices = Vec::with_capacity(prepared.len());

    let apply_result: Result<(), InstallError> = (|| {
        for (index, removal) in prepared.iter().enumerate() {
            if let Some(expected_hash) = &removal.expected_hash {
                revalidate_removal_hash(&removal.absolute, removal.path.as_str(), expected_hash)?;
            }
            std::fs::remove_file(&removal.absolute).map_err(|err| InstallError::RemoveFailure {
                path: removal.path.as_str().to_owned(),
                message: err.to_string(),
            })?;
            match removal.class {
                UninstallRemovalClass::GeneratedAsset => {
                    report.removed_generated.push(removal.path.clone());
                }
                UninstallRemovalClass::InstallMetadata => {
                    report.removed_metadata.push(removal.path.clone());
                }
            }
            changed_indices.push(index);
        }

        if let Some(preview_fingerprint) = preview.manifest_fingerprint() {
            guard_manifest_deletion(&manifest_absolute, preview_fingerprint)?;
        }

        Ok(())
    })();

    if let Err(error) = apply_result {
        return Err(resolve_uninstall_apply_error(
            &repository_root,
            &prepared,
            &changed_indices,
            error,
        ));
    }

    report.sort_paths();
    Ok(report)
}

fn validate_preview_root(
    repository_root: &Path,
    preview: &UninstallPreview,
) -> Result<(), InstallError> {
    if repository_root != preview.repository_root() {
        return Err(InstallError::UninstallPreviewRootMismatch);
    }
    Ok(())
}

fn validate_preview_manifest_fingerprint(
    repository_root: &Path,
    preview: &UninstallPreview,
) -> Result<(), InstallError> {
    let Some(preview_fingerprint) = preview.manifest_fingerprint() else {
        return Ok(());
    };

    let manifest_path = repository_root.join(INSTALL_MANIFEST_REPO_PATH);
    if !manifest_path.exists() {
        return Err(InstallError::UninstallPreviewManifestFingerprintMismatch {
            message: "manifest file no longer exists".to_owned(),
        });
    }

    let current_bytes = std::fs::read(&manifest_path).map_err(|err| InstallError::ReadFailure {
        path: INSTALL_MANIFEST_REPO_PATH.to_owned(),
        message: err.to_string(),
    })?;
    let current_fingerprint = sha256_hex(&current_bytes);
    if current_fingerprint != *preview_fingerprint {
        return Err(InstallError::UninstallPreviewManifestFingerprintMismatch {
            message: "manifest content changed between planning and apply".to_owned(),
        });
    }

    Ok(())
}

fn load_manifest_hashes(
    manifest_absolute: &Path,
) -> Result<Vec<(RepoRelativePath, Sha256Hex)>, InstallError> {
    if !manifest_absolute.exists() {
        return Ok(Vec::new());
    }

    let raw = std::fs::read_to_string(manifest_absolute).map_err(|err| {
        InstallError::InvalidInstallManifest {
            path: INSTALL_MANIFEST_REPO_PATH.to_owned(),
            message: format!("failed to read manifest: {err}"),
        }
    })?;

    let manifest: InstallManifest =
        toml::from_str(&raw).map_err(|err| InstallError::InvalidInstallManifest {
            path: INSTALL_MANIFEST_REPO_PATH.to_owned(),
            message: err.to_string(),
        })?;

    Ok(manifest
        .entries
        .into_iter()
        .map(|entry| (entry.path, entry.content_hash))
        .collect())
}

fn revalidate_removal_hash(
    path: &Path,
    display_path: &str,
    expected_hash: &Sha256Hex,
) -> Result<(), InstallError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|err| InstallError::ReadFailure {
        path: display_path.to_owned(),
        message: err.to_string(),
    })?;

    if metadata.file_type().is_symlink() {
        return Err(InstallError::UninstallRemovalHashDrift {
            path: display_path.to_owned(),
            message: "target is a symbolic link, refusing to remove".to_owned(),
        });
    }

    if !metadata.is_file() {
        return Err(InstallError::UninstallRemovalHashDrift {
            path: display_path.to_owned(),
            message: "target is not a regular file, refusing to remove".to_owned(),
        });
    }

    let observed_hash = compute_file_sha256(path, display_path)?;
    if observed_hash != *expected_hash {
        return Err(InstallError::UninstallRemovalHashDrift {
            path: display_path.to_owned(),
            message: "content hash no longer matches manifest".to_owned(),
        });
    }

    Ok(())
}

fn guard_manifest_deletion(
    manifest_absolute: &Path,
    preview_fingerprint: &Sha256Hex,
) -> Result<(), InstallError> {
    if !manifest_absolute.exists() {
        return Ok(());
    }

    let current_bytes =
        std::fs::read(manifest_absolute).map_err(|err| InstallError::ReadFailure {
            path: INSTALL_MANIFEST_REPO_PATH.to_owned(),
            message: err.to_string(),
        })?;
    let current_fingerprint = sha256_hex(&current_bytes);
    if current_fingerprint != *preview_fingerprint {
        return Err(InstallError::UninstallManifestDrift {
            message: "manifest content changed between planning and manifest deletion".to_owned(),
        });
    }

    Ok(())
}

fn compute_file_sha256(path: &Path, display_path: &str) -> Result<Sha256Hex, InstallError> {
    let mut file = std::fs::File::open(path).map_err(|err| InstallError::ReadFailure {
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
    Ok(sha256_hex(&hasher.finalize()))
}

fn build_uninstall_removal_plan(
    removal_paths: &[RepoRelativePath],
    manifest_exists: bool,
    manifest_path: &RepoRelativePath,
) -> Vec<PlannedUninstallRemoval> {
    let mut planned = Vec::with_capacity(removal_paths.len());
    for path in removal_paths {
        planned.push(PlannedUninstallRemoval {
            path: path.clone(),
            class: UninstallRemovalClass::GeneratedAsset,
        });
    }
    if !manifest_exists {
        return planned;
    }

    match removal_paths.binary_search(manifest_path) {
        Ok(index) => {
            planned[index].class = UninstallRemovalClass::InstallMetadata;
        }
        Err(index) => {
            planned.insert(
                index,
                PlannedUninstallRemoval {
                    path: manifest_path.clone(),
                    class: UninstallRemovalClass::InstallMetadata,
                },
            );
        }
    }

    planned
}

fn prepare_uninstall_apply(
    repository_root: &Path,
    planned_removals: &[PlannedUninstallRemoval],
    manifest_entries: &[(RepoRelativePath, Sha256Hex)],
) -> Result<Vec<PreparedUninstallRemoval>, InstallError> {
    let mut removals = Vec::with_capacity(planned_removals.len());
    for planned in planned_removals {
        let absolute = resolve_repo_path(repository_root, &planned.path)?;
        let metadata = match std::fs::symlink_metadata(&absolute) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == ErrorKind::NotFound => continue,
            Err(err) => {
                return Err(InstallError::ReadFailure {
                    path: planned.path.as_str().to_owned(),
                    message: err.to_string(),
                });
            }
        };
        if !metadata.is_file() {
            return Err(InstallError::RemoveFailure {
                path: planned.path.as_str().to_owned(),
                message: "planned uninstall target is not a regular file".to_owned(),
            });
        }

        let prior_content =
            std::fs::read(&absolute).map_err(|err| InstallError::RemoveFailure {
                path: planned.path.as_str().to_owned(),
                message: err.to_string(),
            })?;

        let expected_hash = manifest_entries
            .iter()
            .find(|(path, _)| path == &planned.path)
            .map(|(_, hash)| hash.clone());

        removals.push(PreparedUninstallRemoval {
            path: planned.path.clone(),
            absolute,
            prior_content,
            class: planned.class,
            expected_hash,
        });
    }
    Ok(removals)
}

fn resolve_uninstall_apply_error(
    repository_root: &Path,
    prepared: &[PreparedUninstallRemoval],
    changed_indices: &[usize],
    cause: InstallError,
) -> InstallError {
    if changed_indices.is_empty() {
        return cause;
    }

    if let Err(rollback_error) = rollback_changed_paths(repository_root, prepared, changed_indices)
    {
        return InstallError::WriteFailure {
            path: ".".to_owned(),
            message: format!("apply failed: {cause}; rollback failed: {rollback_error}"),
        };
    }

    cause
}

fn rollback_changed_paths(
    repository_root: &Path,
    prepared: &[PreparedUninstallRemoval],
    changed_indices: &[usize],
) -> Result<(), InstallError> {
    for index in changed_indices.iter().rev() {
        let Some(removal) = prepared.get(*index) else {
            continue;
        };
        let path = &removal.path;

        let revalidated = resolve_repo_path(repository_root, path)?;
        if revalidated != removal.absolute {
            return Err(InstallError::UnsafeRepositoryPath {
                path: path.as_str().to_owned(),
                message: "resolved path changed between planning and apply".to_owned(),
            });
        }

        atomic_replace_file(path, &revalidated, &removal.prior_content)?;
    }

    Ok(())
}
