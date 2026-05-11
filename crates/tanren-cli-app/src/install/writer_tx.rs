use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::install::error::InstallError;
use crate::install::manifest::RepoRelativePath;
use crate::install::path_guard::{ParentHandle, resolve_repo_path};
use crate::install::plan::{InstallPlan, PlannedWriteKind};

static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub(super) struct PreparedApply {
    pub(super) removals: Vec<PreparedRemoval>,
    pub(super) writes: Vec<PreparedWrite>,
    pub(super) manifest: PreparedManifest,
    pub(super) rollback_records: BTreeMap<RepoRelativePath, RollbackRecord>,
}

#[derive(Debug)]
pub(super) struct PreparedRemoval {
    pub(super) path: RepoRelativePath,
    pub(super) absolute: PathBuf,
    pub(super) parent_handle: ParentHandle,
}

#[derive(Debug)]
pub(super) struct PreparedWrite {
    pub(super) staged: StagedReplacement,
    pub(super) kind: PlannedWriteKind,
}

#[derive(Debug)]
pub(super) struct PreparedManifest {
    pub(super) staged: Option<StagedReplacement>,
    pub(super) prior: Option<Vec<u8>>,
}

#[derive(Debug)]
pub(super) struct StagedReplacement {
    pub(super) path: RepoRelativePath,
    pub(super) absolute: PathBuf,
    pub(super) temp_path: PathBuf,
    pub(super) parent_handle: ParentHandle,
}

#[derive(Debug)]
pub(super) struct RollbackRecord {
    absolute: PathBuf,
    prior: PriorState,
}

#[derive(Debug)]
enum PriorState {
    Missing,
    Present(Vec<u8>),
}

pub(super) fn prepare_apply(plan: &InstallPlan) -> Result<PreparedApply, InstallError> {
    let mut staged_temp_paths = Vec::with_capacity(plan.writes().len() + 1);
    let prepare_result = (|| {
        let manifest_payload = toml::to_string(plan.manifest()).map_err(|err| {
            InstallError::InvalidInstallManifest {
                path: plan.manifest_path().as_str().to_owned(),
                message: err.to_string(),
            }
        })?;
        let manifest_payload = manifest_payload.into_bytes();

        let mut writes = Vec::with_capacity(plan.writes().len());
        let mut rollback_records = BTreeMap::new();
        for write in plan.writes() {
            let staged = stage_replacement_payload(
                plan,
                write.path(),
                write.absolute_path(),
                write.content_bytes(),
            )?;
            staged_temp_paths.push(staged.temp_path.clone());
            let prior = read_prior_payload(write.path(), &staged.absolute)?;
            rollback_records.insert(
                write.path().clone(),
                RollbackRecord {
                    absolute: staged.absolute.clone(),
                    prior: prior_to_state(prior.clone()),
                },
            );
            writes.push(PreparedWrite {
                staged,
                kind: write.kind(),
            });
        }

        let manifest_absolute = revalidate_planned_apply_path(
            plan,
            plan.manifest_path(),
            plan.manifest_absolute_path(),
        )?;
        ensure_destination_not_symlink(plan.manifest_path(), &manifest_absolute)?;
        let manifest_prior = read_prior_payload(plan.manifest_path(), &manifest_absolute)?;
        let staged_manifest = if manifest_prior.as_deref() == Some(manifest_payload.as_slice()) {
            None
        } else {
            let staged = stage_replacement_payload(
                plan,
                plan.manifest_path(),
                plan.manifest_absolute_path(),
                &manifest_payload,
            )?;
            staged_temp_paths.push(staged.temp_path.clone());
            rollback_records.insert(
                plan.manifest_path().clone(),
                RollbackRecord {
                    absolute: staged.absolute.clone(),
                    prior: prior_to_state(manifest_prior.clone()),
                },
            );
            Some(staged)
        };
        let manifest = PreparedManifest {
            staged: staged_manifest,
            prior: manifest_prior,
        };

        let mut removals = Vec::with_capacity(plan.removals().len());
        for removal in plan.removals() {
            let absolute =
                revalidate_planned_apply_path(plan, removal.path(), removal.absolute_path())?;
            let prior = fs::read(&absolute).map_err(|err| InstallError::RemoveFailure {
                path: removal.path().as_str().to_owned(),
                message: err.to_string(),
            })?;
            rollback_records.insert(
                removal.path().clone(),
                RollbackRecord {
                    absolute: absolute.clone(),
                    prior: PriorState::Present(prior),
                },
            );
            let parent_handle = ParentHandle::open(removal.path(), &absolute)?;
            removals.push(PreparedRemoval {
                path: removal.path().clone(),
                absolute,
                parent_handle,
            });
        }

        Ok(PreparedApply {
            removals,
            writes,
            manifest,
            rollback_records,
        })
    })();

    if prepare_result.is_err() {
        for path in &staged_temp_paths {
            cleanup_temporary_file(path);
        }
    }

    prepare_result
}

fn prior_to_state(prior: Option<Vec<u8>>) -> PriorState {
    match prior {
        Some(bytes) => PriorState::Present(bytes),
        None => PriorState::Missing,
    }
}

fn read_prior_payload(
    path: &RepoRelativePath,
    absolute: &Path,
) -> Result<Option<Vec<u8>>, InstallError> {
    match fs::read(absolute) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(InstallError::ReadFailure {
            path: path.as_str().to_owned(),
            message: err.to_string(),
        }),
    }
}

fn stage_replacement_payload(
    plan: &InstallPlan,
    path: &RepoRelativePath,
    planned_absolute: &Path,
    content: &[u8],
) -> Result<StagedReplacement, InstallError> {
    let absolute = revalidate_planned_apply_path(plan, path, planned_absolute)?;
    ensure_parent_directory(path, &absolute)?;
    let absolute = revalidate_planned_apply_path(plan, path, planned_absolute)?;
    let temp_path = create_staged_temporary_payload(path, &absolute, content)?;
    let parent_handle = ParentHandle::open(path, &absolute)?;
    Ok(StagedReplacement {
        path: path.clone(),
        absolute,
        temp_path,
        parent_handle,
    })
}

fn create_staged_temporary_payload(
    path: &RepoRelativePath,
    absolute: &Path,
    content: &[u8],
) -> Result<PathBuf, InstallError> {
    ensure_destination_not_symlink(path, absolute)?;

    let Some(parent) = absolute.parent() else {
        return Err(InstallError::WriteFailure {
            path: path.as_str().to_owned(),
            message: "destination path has no parent directory".to_owned(),
        });
    };

    let temp = create_temp_file(path, parent, absolute)?;
    let mut file = temp.file;

    if let Err(err) = file.write_all(content) {
        cleanup_temporary_file(&temp.path);
        return Err(InstallError::WriteFailure {
            path: path.as_str().to_owned(),
            message: err.to_string(),
        });
    }

    if let Err(err) = file.sync_all() {
        cleanup_temporary_file(&temp.path);
        return Err(InstallError::WriteFailure {
            path: path.as_str().to_owned(),
            message: err.to_string(),
        });
    }

    drop(file);
    Ok(temp.path)
}

pub(super) fn commit_staged_replacement(
    plan: &InstallPlan,
    staged: &StagedReplacement,
) -> Result<(), InstallError> {
    let absolute = revalidate_planned_apply_path(plan, &staged.path, &staged.absolute)?;
    ensure_destination_not_symlink(&staged.path, &absolute)?;
    staged.parent_handle.verify(&staged.path, &absolute)?;
    fs::rename(&staged.temp_path, &absolute).map_err(|err| {
        cleanup_temporary_file(&staged.temp_path);
        InstallError::WriteFailure {
            path: staged.path.as_str().to_owned(),
            message: err.to_string(),
        }
    })
}

pub(super) fn commit_staged_removal(
    relative: &RepoRelativePath,
    absolute: &Path,
    handle: &ParentHandle,
) -> Result<(), InstallError> {
    handle.verify(relative, absolute)?;
    fs::remove_file(absolute).map_err(|err| InstallError::RemoveFailure {
        path: relative.as_str().to_owned(),
        message: err.to_string(),
    })
}

pub(super) fn cleanup_staged_payloads(prepared: &PreparedApply) {
    for write in &prepared.writes {
        cleanup_temporary_file(&write.staged.temp_path);
    }
    if let Some(staged_manifest) = &prepared.manifest.staged {
        cleanup_temporary_file(&staged_manifest.temp_path);
    }
}

pub(super) fn resolve_apply_failure(
    plan: &InstallPlan,
    rollback_records: &BTreeMap<RepoRelativePath, RollbackRecord>,
    changed_paths: &[RepoRelativePath],
    cause: InstallError,
) -> InstallError {
    if changed_paths.is_empty() {
        return cause;
    }

    if let Err(rollback_error) = rollback_changed_paths(plan, rollback_records, changed_paths) {
        return InstallError::WriteFailure {
            path: ".".to_owned(),
            message: format!("apply failed: {cause}; rollback failed: {rollback_error}"),
        };
    }

    cause
}

fn rollback_changed_paths(
    plan: &InstallPlan,
    rollback_records: &BTreeMap<RepoRelativePath, RollbackRecord>,
    changed_paths: &[RepoRelativePath],
) -> Result<(), InstallError> {
    for path in changed_paths.iter().rev() {
        let Some(record) = rollback_records.get(path) else {
            continue;
        };
        let absolute = revalidate_planned_apply_path(plan, path, &record.absolute)?;
        match &record.prior {
            PriorState::Missing => remove_if_present(path, &absolute)?,
            PriorState::Present(bytes) => {
                ensure_parent_directory(path, &absolute)?;
                atomic_replace_file(path, &absolute, bytes)?;
            }
        }
    }

    Ok(())
}

fn remove_if_present(relative_path: &RepoRelativePath, path: &Path) -> Result<(), InstallError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(InstallError::WriteFailure {
            path: relative_path.as_str().to_owned(),
            message: err.to_string(),
        }),
    }
}

fn resolve_apply_path(
    plan: &InstallPlan,
    path: &RepoRelativePath,
) -> Result<PathBuf, InstallError> {
    resolve_repo_path(plan.repository_root(), path)
}

fn revalidate_planned_apply_path(
    plan: &InstallPlan,
    path: &RepoRelativePath,
    planned_absolute: &Path,
) -> Result<PathBuf, InstallError> {
    let absolute = resolve_apply_path(plan, path)?;
    if planned_absolute != absolute {
        return Err(InstallError::UnsafeRepositoryPath {
            path: path.as_str().to_owned(),
            message: "resolved path changed between planning and apply".to_owned(),
        });
    }
    Ok(absolute)
}

fn ensure_parent_directory(path: &RepoRelativePath, absolute: &Path) -> Result<(), InstallError> {
    if let Some(parent) = absolute.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent).map_err(|err| InstallError::CreateDirectoryFailure {
            path: path.as_str().to_owned(),
            message: err.to_string(),
        })?;
    }

    Ok(())
}

fn atomic_replace_file(
    path: &RepoRelativePath,
    absolute: &Path,
    content: &[u8],
) -> Result<(), InstallError> {
    ensure_destination_not_symlink(path, absolute)?;

    let Some(parent) = absolute.parent() else {
        return Err(InstallError::WriteFailure {
            path: path.as_str().to_owned(),
            message: "destination path has no parent directory".to_owned(),
        });
    };

    let temp = create_temp_file(path, parent, absolute)?;
    let mut file = temp.file;

    if let Err(err) = file.write_all(content) {
        cleanup_temporary_file(&temp.path);
        return Err(InstallError::WriteFailure {
            path: path.as_str().to_owned(),
            message: err.to_string(),
        });
    }

    if let Err(err) = file.sync_all() {
        cleanup_temporary_file(&temp.path);
        return Err(InstallError::WriteFailure {
            path: path.as_str().to_owned(),
            message: err.to_string(),
        });
    }

    drop(file);
    fs::rename(&temp.path, absolute).map_err(|err| {
        cleanup_temporary_file(&temp.path);
        InstallError::WriteFailure {
            path: path.as_str().to_owned(),
            message: err.to_string(),
        }
    })
}

fn ensure_destination_not_symlink(
    path: &RepoRelativePath,
    absolute: &Path,
) -> Result<(), InstallError> {
    if let Ok(metadata) = fs::symlink_metadata(absolute)
        && metadata.file_type().is_symlink()
    {
        return Err(InstallError::UnsafeRepositoryPath {
            path: path.as_str().to_owned(),
            message: "resolved destination is a symbolic link".to_owned(),
        });
    }

    Ok(())
}

struct TemporaryFile {
    path: PathBuf,
    file: fs::File,
}

fn create_temp_file(
    path: &RepoRelativePath,
    parent: &Path,
    destination: &Path,
) -> Result<TemporaryFile, InstallError> {
    let mut last_message = String::from("failed allocating temporary file path");

    for attempt in 0..64 {
        let temp_path = build_temp_path(parent, destination, attempt);
        match OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
        {
            Ok(file) => {
                return Ok(TemporaryFile {
                    path: temp_path,
                    file,
                });
            }
            Err(err) if err.kind() == ErrorKind::AlreadyExists => {
                last_message = err.to_string();
            }
            Err(err) => {
                return Err(InstallError::WriteFailure {
                    path: path.as_str().to_owned(),
                    message: err.to_string(),
                });
            }
        }
    }

    Err(InstallError::WriteFailure {
        path: path.as_str().to_owned(),
        message: format!("temporary file allocation exhausted retries: {last_message}"),
    })
}

fn build_temp_path(parent: &Path, destination: &Path, attempt: u32) -> PathBuf {
    let now_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let destination_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("tanren-install");
    let temp_name = format!(
        ".{destination_name}.tanren-install-{}-{sequence}-{now_nanos}-{attempt}.tmp",
        std::process::id(),
    );
    parent.join(temp_name)
}

fn cleanup_temporary_file(path: &Path) {
    if path.exists() {
        let _ = fs::remove_file(path);
    }
}
