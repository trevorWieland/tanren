//! Filesystem writer for manifest-driven install plans.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::install::error::InstallError;
use crate::install::manifest::{INSTALL_MANIFEST_REPO_PATH, RepoRelativePath};
use crate::install::path_guard::resolve_repo_path;
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
}

impl UninstallApplyReport {
    fn sort_paths(&mut self) {
        self.removed_generated.sort();
        self.removed_metadata.sort();
    }
}

#[derive(Debug, Clone)]
struct PreparedUninstallRemoval {
    path: RepoRelativePath,
    absolute: PathBuf,
    prior_content: Vec<u8>,
    class: UninstallRemovalClass,
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
    let manifest_path = RepoRelativePath::parse(INSTALL_MANIFEST_REPO_PATH)?;
    let manifest_absolute = resolve_manifest_absolute_path(&repository_root, &manifest_path)?;
    let planned_removals =
        build_uninstall_removal_plan(preview.remove(), manifest_absolute.exists(), &manifest_path);
    if planned_removals.is_empty() {
        return Ok(UninstallApplyReport::default());
    }

    let prepared = prepare_uninstall_apply(&repository_root, &planned_removals)?;
    let mut report = UninstallApplyReport::default();
    let mut changed_indices = Vec::with_capacity(prepared.len());

    let apply_result: Result<(), InstallError> = (|| {
        for (index, removal) in prepared.iter().enumerate() {
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

fn build_uninstall_removal_plan(
    removal_paths: &[RepoRelativePath],
    manifest_exists: bool,
    manifest_path: &RepoRelativePath,
) -> Vec<PlannedUninstallRemoval> {
    let mut planned = Vec::with_capacity(removal_paths.len() + usize::from(manifest_exists));
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

fn resolve_manifest_absolute_path(
    repository_root: &Path,
    manifest_path: &RepoRelativePath,
) -> Result<PathBuf, InstallError> {
    resolve_repo_path(repository_root, manifest_path)
}

fn prepare_uninstall_apply(
    repository_root: &Path,
    planned_removals: &[PlannedUninstallRemoval],
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
        removals.push(PreparedUninstallRemoval {
            path: planned.path.clone(),
            absolute,
            prior_content,
            class: planned.class,
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

fn display_repository_argument(path: &Path) -> String {
    if path.is_absolute() {
        "<redacted-absolute-path>".to_owned()
    } else {
        path.display().to_string()
    }
}

fn redacted_io_error_kind(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::NotFound => "not_found",
        ErrorKind::PermissionDenied => "permission_denied",
        ErrorKind::AlreadyExists => "already_exists",
        ErrorKind::InvalidInput => "invalid_input",
        ErrorKind::InvalidData => "invalid_data",
        ErrorKind::TimedOut => "timed_out",
        ErrorKind::WriteZero => "write_zero",
        ErrorKind::Interrupted => "interrupted",
        ErrorKind::Unsupported => "unsupported",
        ErrorKind::UnexpectedEof => "unexpected_eof",
        _ => "io_error",
    }
}
