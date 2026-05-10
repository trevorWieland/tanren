//! Filesystem writer for manifest-driven install plans.

use std::path::{Path, PathBuf};

use crate::install::error::InstallError;
use crate::install::manifest::{INSTALL_MANIFEST_REPO_PATH, RepoRelativePath};
use crate::install::path_guard::resolve_repo_path;
use crate::install::plan::{InstallPlan, PlannedWriteKind};
use crate::install::uninstall_plan::UninstallPreview;
use crate::install::writer_tx::{
    cleanup_staged_payloads, commit_staged_replacement, prepare_apply, resolve_apply_failure,
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
    let mut removals = preview.remove().to_vec();
    let manifest_absolute = resolve_manifest_absolute_path(&repository_root, &manifest_path)?;
    if manifest_absolute.exists() {
        removals.push(manifest_path.clone());
    }
    removals.sort();
    removals.dedup();

    let prepared = prepare_uninstall_apply(&repository_root, &removals)?;
    let mut report = UninstallApplyReport::default();
    let mut changed_paths = Vec::new();

    let apply_result: Result<(), InstallError> = (|| {
        for removal in &prepared {
            std::fs::remove_file(&removal.absolute).map_err(|err| InstallError::RemoveFailure {
                path: removal.path.as_str().to_owned(),
                message: err.to_string(),
            })?;

            if removal.path == manifest_path {
                report.removed_metadata.push(removal.path.clone());
            } else {
                report.removed_generated.push(removal.path.clone());
            }
            changed_paths.push(removal.path.clone());
        }

        Ok(())
    })();

    if let Err(error) = apply_result {
        return Err(resolve_uninstall_apply_error(
            &repository_root,
            &prepared,
            &changed_paths,
            error,
        ));
    }

    report.sort_paths();
    Ok(report)
}

fn validate_repository_root(repository: &Path) -> Result<PathBuf, InstallError> {
    let canonical =
        repository
            .canonicalize()
            .map_err(|err| InstallError::InvalidRepositoryPath {
                path: format!("{} ({err})", display_repository_argument(repository)),
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
    removal_paths: &[RepoRelativePath],
) -> Result<Vec<PreparedUninstallRemoval>, InstallError> {
    let mut removals = Vec::with_capacity(removal_paths.len());
    for path in removal_paths {
        let absolute = resolve_repo_path(repository_root, path)?;
        if !absolute.exists() {
            continue;
        }

        let metadata = std::fs::metadata(&absolute).map_err(|err| InstallError::ReadFailure {
            path: path.as_str().to_owned(),
            message: err.to_string(),
        })?;
        if !metadata.is_file() {
            return Err(InstallError::RemoveFailure {
                path: path.as_str().to_owned(),
                message: "planned uninstall target is not a regular file".to_owned(),
            });
        }

        let prior_content =
            std::fs::read(&absolute).map_err(|err| InstallError::RemoveFailure {
                path: path.as_str().to_owned(),
                message: err.to_string(),
            })?;
        removals.push(PreparedUninstallRemoval {
            path: path.clone(),
            absolute,
            prior_content,
        });
    }
    Ok(removals)
}

fn resolve_uninstall_apply_error(
    repository_root: &Path,
    prepared: &[PreparedUninstallRemoval],
    changed_paths: &[RepoRelativePath],
    cause: InstallError,
) -> InstallError {
    if changed_paths.is_empty() {
        return cause;
    }

    if let Err(rollback_error) = rollback_changed_paths(repository_root, prepared, changed_paths) {
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
    changed_paths: &[RepoRelativePath],
) -> Result<(), InstallError> {
    for path in changed_paths.iter().rev() {
        let Some(removal) = prepared.iter().find(|candidate| &candidate.path == path) else {
            continue;
        };

        let revalidated = resolve_repo_path(repository_root, path)?;
        if revalidated != removal.absolute {
            return Err(InstallError::UnsafeRepositoryPath {
                path: path.as_str().to_owned(),
                message: "resolved path changed between planning and apply".to_owned(),
            });
        }

        if let Some(parent) = revalidated.parent()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent).map_err(|err| {
                InstallError::CreateDirectoryFailure {
                    path: path.as_str().to_owned(),
                    message: err.to_string(),
                }
            })?;
        }

        std::fs::write(&revalidated, &removal.prior_content).map_err(|err| {
            InstallError::WriteFailure {
                path: path.as_str().to_owned(),
                message: err.to_string(),
            }
        })?;
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
