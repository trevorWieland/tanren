//! Filesystem writer for manifest-driven install plans.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::install::error::InstallError;
use crate::install::manifest::RepoRelativePath;
use crate::install::path_guard::resolve_repo_path;
use crate::install::plan::{InstallPlan, PlannedRemoval, PlannedWrite, PlannedWriteKind};

static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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

/// Apply a previously validated install plan.
pub fn apply_install_plan(plan: &InstallPlan) -> Result<InstallReport, InstallError> {
    let mut report = InstallReport {
        preserved: plan.preserved().to_vec(),
        ..InstallReport::default()
    };

    for removal in plan.removals() {
        remove_file(plan, removal)?;
        report.removed.push(removal.path().clone());
    }

    for write in plan.writes() {
        write_file(plan, write)?;
        match write.kind() {
            PlannedWriteKind::Created => report.created.push(write.path().clone()),
            PlannedWriteKind::Updated => report.updated.push(write.path().clone()),
            PlannedWriteKind::Restored => report.restored.push(write.path().clone()),
        }
    }

    write_manifest(plan, &mut report)?;
    report.sort_paths();
    Ok(report)
}

fn write_manifest(plan: &InstallPlan, report: &mut InstallReport) -> Result<(), InstallError> {
    let manifest_path = plan.manifest_path();
    let absolute = resolve_apply_path(plan, manifest_path)?;
    ensure_revalidated_matches_planned(manifest_path, plan.manifest_absolute_path(), &absolute)?;
    let prior = fs::read(&absolute).ok();

    let payload =
        toml::to_string(plan.manifest()).map_err(|err| InstallError::InvalidInstallManifest {
            path: absolute.display().to_string(),
            message: err.to_string(),
        })?;

    write_repo_bytes(
        plan,
        manifest_path,
        plan.manifest_absolute_path(),
        payload.as_bytes(),
    )?;

    match prior {
        None => report.created.push(manifest_path.clone()),
        Some(previous_payload) if previous_payload != payload.as_bytes() => {
            report.updated.push(manifest_path.clone());
        }
        Some(_) => {}
    }

    Ok(())
}

fn resolve_apply_path(
    plan: &InstallPlan,
    path: &RepoRelativePath,
) -> Result<PathBuf, InstallError> {
    resolve_repo_path(plan.repository_root(), path)
}

fn remove_file(plan: &InstallPlan, removal: &PlannedRemoval) -> Result<(), InstallError> {
    let absolute = resolve_apply_path(plan, removal.path())?;
    ensure_revalidated_matches_planned(removal.path(), removal.absolute_path(), &absolute)?;
    fs::remove_file(&absolute).map_err(|err| InstallError::RemoveFailure {
        path: absolute.display().to_string(),
        message: err.to_string(),
    })
}

fn write_file(plan: &InstallPlan, write: &PlannedWrite) -> Result<(), InstallError> {
    write_repo_bytes(
        plan,
        write.path(),
        write.absolute_path(),
        write.content_bytes(),
    )
}

fn write_repo_bytes(
    plan: &InstallPlan,
    path: &RepoRelativePath,
    planned_absolute: &Path,
    content: &[u8],
) -> Result<(), InstallError> {
    let absolute = resolve_apply_path(plan, path)?;
    ensure_revalidated_matches_planned(path, planned_absolute, &absolute)?;
    ensure_parent_directory(&absolute)?;
    let absolute = resolve_apply_path(plan, path)?;
    ensure_revalidated_matches_planned(path, planned_absolute, &absolute)?;
    atomic_replace_file(path, &absolute, content)
}

fn ensure_revalidated_matches_planned(
    path: &RepoRelativePath,
    planned: &Path,
    revalidated: &Path,
) -> Result<(), InstallError> {
    if planned != revalidated {
        return Err(InstallError::UnsafeRepositoryPath {
            path: path.as_str().to_owned(),
            message: format!(
                "resolved path changed between planning and apply: planned='{}' apply='{}'",
                planned.display(),
                revalidated.display(),
            ),
        });
    }

    Ok(())
}

fn ensure_parent_directory(absolute: &Path) -> Result<(), InstallError> {
    if let Some(parent) = absolute.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent).map_err(|err| InstallError::CreateDirectoryFailure {
            path: parent.display().to_string(),
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
    if let Ok(metadata) = fs::symlink_metadata(absolute) {
        if metadata.file_type().is_symlink() {
            return Err(InstallError::UnsafeRepositoryPath {
                path: path.as_str().to_owned(),
                message: "resolved destination is a symbolic link".to_owned(),
            });
        }
    }

    let Some(parent) = absolute.parent() else {
        return Err(InstallError::WriteFailure {
            path: absolute.display().to_string(),
            message: "destination path has no parent directory".to_owned(),
        });
    };

    let temp = create_temp_file(parent, absolute)?;
    let mut file = temp.file;

    if let Err(err) = file.write_all(content) {
        cleanup_temporary_file(&temp.path);
        return Err(InstallError::WriteFailure {
            path: absolute.display().to_string(),
            message: err.to_string(),
        });
    }

    if let Err(err) = file.sync_all() {
        cleanup_temporary_file(&temp.path);
        return Err(InstallError::WriteFailure {
            path: absolute.display().to_string(),
            message: err.to_string(),
        });
    }

    drop(file);
    fs::rename(&temp.path, absolute).map_err(|err| {
        cleanup_temporary_file(&temp.path);
        InstallError::WriteFailure {
            path: absolute.display().to_string(),
            message: err.to_string(),
        }
    })
}

struct TemporaryFile {
    path: PathBuf,
    file: fs::File,
}

fn create_temp_file(parent: &Path, destination: &Path) -> Result<TemporaryFile, InstallError> {
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
                    path: destination.display().to_string(),
                    message: err.to_string(),
                });
            }
        }
    }

    Err(InstallError::WriteFailure {
        path: destination.display().to_string(),
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
