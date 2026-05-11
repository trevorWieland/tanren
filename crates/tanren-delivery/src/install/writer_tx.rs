//! Transactional install apply: prepare, commit, and rollback primitives.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};

use crate::install::error::InstallError;
use crate::install::manifest::RepoRelativePath;
use crate::install::plan::{InstallPlan, PlannedWriteKind};
use crate::install::writer_tx_support::{
    cleanup_temporary_file, create_temp_file, ensure_parent_directory,
    guard_destination_for_mutation, remove_with_verification, rename_with_verification,
};

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
}

#[derive(Debug)]
pub(super) struct PreparedWrite {
    pub(super) staged: StagedReplacement,
    pub(super) kind: PlannedWriteKind,
}

#[derive(Debug)]
pub(super) struct PreparedManifest {
    pub(super) staged: Option<StagedReplacement>,
    pub(super) change: ManifestChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ManifestChange {
    Created,
    Updated,
    Unchanged,
}

#[derive(Debug)]
pub(super) struct StagedReplacement {
    pub(super) path: RepoRelativePath,
    pub(super) absolute: PathBuf,
    pub(super) temp_path: PathBuf,
    pub(super) parent: PathBuf,
}

#[derive(Debug)]
pub(super) struct RollbackRecord {
    absolute: PathBuf,
    prior: PriorState,
}

#[derive(Debug)]
enum PriorState {
    Missing,
    Scratch(PathBuf),
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
            let prior = snapshot_prior_state(plan, write.path(), write.absolute_path())?;
            rollback_records.insert(
                write.path().clone(),
                RollbackRecord {
                    absolute: staged.absolute.clone(),
                    prior,
                },
            );
            writes.push(PreparedWrite {
                staged,
                kind: write.kind(),
            });
        }

        let manifest = prepare_manifest_apply(
            plan,
            &manifest_payload.into_bytes(),
            &mut rollback_records,
            &mut staged_temp_paths,
        )?;

        let mut removals = Vec::with_capacity(plan.removals().len());
        for removal in plan.removals() {
            let guard =
                guard_destination_for_mutation(plan, removal.path(), removal.absolute_path())?;
            let prior = snapshot_prior_state(plan, removal.path(), removal.absolute_path())?;
            rollback_records.insert(
                removal.path().clone(),
                RollbackRecord {
                    absolute: guard.absolute.clone(),
                    prior,
                },
            );
            removals.push(PreparedRemoval {
                path: removal.path().clone(),
                absolute: guard.absolute,
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

fn prepare_manifest_apply(
    plan: &InstallPlan,
    payload: &[u8],
    rollback_records: &mut BTreeMap<RepoRelativePath, RollbackRecord>,
    staged_temp_paths: &mut Vec<PathBuf>,
) -> Result<PreparedManifest, InstallError> {
    let path = plan.manifest_path();
    let absolute = plan.manifest_absolute_path();
    let guard = guard_destination_for_mutation(plan, path, absolute)?;

    let change = if guard.destination_exists {
        if file_contents_match(path, absolute, payload)? {
            ManifestChange::Unchanged
        } else {
            ManifestChange::Updated
        }
    } else {
        ManifestChange::Created
    };

    if change == ManifestChange::Unchanged {
        return Ok(PreparedManifest {
            staged: None,
            change,
        });
    }

    let staged = stage_replacement_payload(plan, path, absolute, payload)?;
    staged_temp_paths.push(staged.temp_path.clone());
    let prior = snapshot_prior_state(plan, path, absolute)?;
    rollback_records.insert(
        path.clone(),
        RollbackRecord {
            absolute: staged.absolute.clone(),
            prior,
        },
    );

    Ok(PreparedManifest {
        staged: Some(staged),
        change,
    })
}

fn file_contents_match(
    path: &RepoRelativePath,
    absolute: &Path,
    expected: &[u8],
) -> Result<bool, InstallError> {
    let mut file = fs::File::open(absolute).map_err(|err| InstallError::ReadFailure {
        path: path.as_str().to_owned(),
        message: err.to_string(),
    })?;

    let actual_size = file.metadata().map_err(|err| InstallError::ReadFailure {
        path: path.as_str().to_owned(),
        message: err.to_string(),
    })?;
    let expected_len = u64::try_from(expected.len()).map_err(|_| InstallError::ReadFailure {
        path: path.as_str().to_owned(),
        message: "manifest payload length overflow".to_owned(),
    })?;
    if actual_size.len() != expected_len {
        return Ok(false);
    }

    let mut offset = 0;
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|err| InstallError::ReadFailure {
                path: path.as_str().to_owned(),
                message: err.to_string(),
            })?;
        if read == 0 {
            break;
        }

        let next = offset + read;
        if expected.get(offset..next) != Some(&buffer[..read]) {
            return Ok(false);
        }
        offset = next;
    }

    Ok(offset == expected.len())
}

/// Snapshot the prior state of a destination path by streaming its content
/// to a scratch file. The file is copied using streaming I/O to avoid
/// retaining large file contents in memory.
fn snapshot_prior_state(
    plan: &InstallPlan,
    path: &RepoRelativePath,
    planned_absolute: &Path,
) -> Result<PriorState, InstallError> {
    let guard = guard_destination_for_mutation(plan, path, planned_absolute)?;
    if !guard.destination_exists {
        return Ok(PriorState::Missing);
    }

    let scratch = stream_to_rollback_scratch(path, &guard.parent, &guard.absolute)?;
    Ok(PriorState::Scratch(scratch))
}

fn stage_replacement_payload(
    plan: &InstallPlan,
    path: &RepoRelativePath,
    planned_absolute: &Path,
    content: &[u8],
) -> Result<StagedReplacement, InstallError> {
    let guard = guard_destination_for_mutation(plan, path, planned_absolute)?;
    ensure_parent_directory(path, &guard.parent)?;
    let guard = guard_destination_for_mutation(plan, path, planned_absolute)?;
    let temp_path = create_staged_temporary_payload(path, &guard.parent, &guard.absolute, content)?;

    Ok(StagedReplacement {
        path: path.clone(),
        absolute: guard.absolute,
        temp_path,
        parent: guard.parent,
    })
}

fn create_staged_temporary_payload(
    path: &RepoRelativePath,
    parent: &Path,
    destination: &Path,
    content: &[u8],
) -> Result<PathBuf, InstallError> {
    let temp = create_temp_file(path, parent, destination)?;
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

/// Stream prior file content to a scratch file for rollback.
///
/// Uses `fs::copy` which performs streaming I/O internally (`copy_file_range`
/// on Linux), avoiding loading the entire file into heap memory. The scratch
/// file is fsynced before returning to ensure durability.
fn stream_to_rollback_scratch(
    path: &RepoRelativePath,
    parent: &Path,
    source: &Path,
) -> Result<PathBuf, InstallError> {
    let temp = create_temp_file(path, parent, source)?;
    let temp_path = temp.path;

    fs::copy(source, &temp_path).map_err(|err| {
        cleanup_temporary_file(&temp_path);
        InstallError::ReadFailure {
            path: path.as_str().to_owned(),
            message: err.to_string(),
        }
    })?;

    fs::File::open(&temp_path)
        .and_then(|file| file.sync_all())
        .map_err(|err| {
            cleanup_temporary_file(&temp_path);
            InstallError::WriteFailure {
                path: path.as_str().to_owned(),
                message: err.to_string(),
            }
        })?;

    Ok(temp_path)
}

/// Commit a staged replacement using hardened rename-with-verification.
///
/// Tracks the parent directory in `touched_parents` for batched fsync.
pub(super) fn commit_staged_replacement(
    plan: &InstallPlan,
    staged: &StagedReplacement,
    touched_parents: &mut BTreeSet<PathBuf>,
) -> Result<(), InstallError> {
    let _ = guard_destination_for_mutation(plan, &staged.path, &staged.absolute)?;
    rename_with_verification(
        &staged.path,
        &staged.temp_path,
        &staged.absolute,
        &staged.parent,
        touched_parents,
    )
}

/// Remove a prepared file using hardened remove-with-verification.
///
/// Tracks the parent directory in `touched_parents` for batched fsync.
pub(super) fn remove_prepared_file(
    plan: &InstallPlan,
    path: &RepoRelativePath,
    planned_absolute: &Path,
    touched_parents: &mut BTreeSet<PathBuf>,
) -> Result<(), InstallError> {
    let guard = guard_destination_for_mutation(plan, path, planned_absolute)?;
    remove_with_verification(path, &guard.absolute, &guard.parent, touched_parents)
}

pub(super) fn cleanup_staged_payloads(prepared: &PreparedApply) {
    for write in &prepared.writes {
        cleanup_temporary_file(&write.staged.temp_path);
    }
    if let Some(manifest) = &prepared.manifest.staged {
        cleanup_temporary_file(&manifest.temp_path);
    }
}

/// Clean up rollback scratch files on both success and rollback failure paths.
///
/// Scratch files hold streamed prior content and must be removed regardless
/// of whether the apply succeeded or rollback completed.
pub(super) fn cleanup_rollback_scratch_files(
    rollback_records: &BTreeMap<RepoRelativePath, RollbackRecord>,
) {
    for record in rollback_records.values() {
        if let PriorState::Scratch(path) = &record.prior {
            cleanup_temporary_file(path);
        }
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

        match &record.prior {
            PriorState::Missing => remove_if_present(plan, path, &record.absolute)?,
            PriorState::Scratch(scratch) => {
                restore_from_scratch(plan, path, &record.absolute, scratch)?;
            }
        }
    }

    Ok(())
}

fn remove_if_present(
    plan: &InstallPlan,
    path: &RepoRelativePath,
    planned_absolute: &Path,
) -> Result<(), InstallError> {
    let guard = guard_destination_for_mutation(plan, path, planned_absolute)?;
    match fs::remove_file(&guard.absolute) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(InstallError::WriteFailure {
            path: path.as_str().to_owned(),
            message: err.to_string(),
        }),
    }
}

fn restore_from_scratch(
    plan: &InstallPlan,
    path: &RepoRelativePath,
    planned_absolute: &Path,
    scratch: &Path,
) -> Result<(), InstallError> {
    let guard = guard_destination_for_mutation(plan, path, planned_absolute)?;
    ensure_parent_directory(path, &guard.parent)?;
    let guard = guard_destination_for_mutation(plan, path, planned_absolute)?;

    let temp_path = create_temp_file(path, &guard.parent, &guard.absolute)?.path;
    fs::copy(scratch, &temp_path).map_err(|err| {
        cleanup_temporary_file(&temp_path);
        InstallError::WriteFailure {
            path: path.as_str().to_owned(),
            message: err.to_string(),
        }
    })?;

    fs::rename(&temp_path, &guard.absolute).map_err(|err| {
        cleanup_temporary_file(&temp_path);
        InstallError::WriteFailure {
            path: path.as_str().to_owned(),
            message: err.to_string(),
        }
    })
}
