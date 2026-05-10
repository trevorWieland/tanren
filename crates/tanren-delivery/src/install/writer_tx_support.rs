use std::fs::{self, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::install::error::InstallError;
use crate::install::manifest::RepoRelativePath;
use crate::install::path_guard::{resolve_repo_parent_path, resolve_repo_path};
use crate::install::plan::InstallPlan;

static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub(super) struct GuardedDestination {
    pub(super) absolute: PathBuf,
    pub(super) parent: PathBuf,
    pub(super) destination_exists: bool,
}

#[derive(Debug)]
pub(super) struct TemporaryFile {
    pub(super) path: PathBuf,
    pub(super) file: fs::File,
}

pub(super) fn guard_destination_for_mutation(
    plan: &InstallPlan,
    path: &RepoRelativePath,
    planned_absolute: &Path,
) -> Result<GuardedDestination, InstallError> {
    let absolute = revalidate_planned_apply_path(plan, path, planned_absolute)?;
    let parent = resolve_parent_directory(plan, path, &absolute)?;

    let destination_exists = match fs::symlink_metadata(&absolute) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(InstallError::UnsafeRepositoryPath {
                    path: path.as_str().to_owned(),
                    message: "resolved destination is a symbolic link".to_owned(),
                });
            }
            true
        }
        Err(err) if err.kind() == ErrorKind::NotFound => false,
        Err(err) => {
            return Err(InstallError::ReadFailure {
                path: path.as_str().to_owned(),
                message: err.to_string(),
            });
        }
    };

    Ok(GuardedDestination {
        absolute,
        parent,
        destination_exists,
    })
}

pub(super) fn ensure_parent_directory(
    path: &RepoRelativePath,
    parent: &Path,
) -> Result<(), InstallError> {
    if !parent.exists() {
        fs::create_dir_all(parent).map_err(|err| InstallError::CreateDirectoryFailure {
            path: path.as_str().to_owned(),
            message: err.to_string(),
        })?;
    }

    Ok(())
}

pub(super) fn create_temp_file(
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

pub(super) fn cleanup_temporary_file(path: &Path) {
    if path.exists() {
        let _ = fs::remove_file(path);
    }
}

fn resolve_parent_directory(
    plan: &InstallPlan,
    path: &RepoRelativePath,
    absolute: &Path,
) -> Result<PathBuf, InstallError> {
    let Some(parent_absolute) = absolute.parent() else {
        return Err(InstallError::WriteFailure {
            path: path.as_str().to_owned(),
            message: "destination path has no parent directory".to_owned(),
        });
    };

    let parent_revalidated = if let Some(parent_path) = resolve_repo_parent_path(path)? {
        resolve_apply_path(plan, &parent_path)?
    } else {
        plan.repository_root().to_path_buf()
    };

    if parent_revalidated != parent_absolute {
        return Err(InstallError::UnsafeRepositoryPath {
            path: path.as_str().to_owned(),
            message: "resolved parent path changed between planning and apply".to_owned(),
        });
    }

    if let Ok(metadata) = fs::symlink_metadata(parent_absolute) {
        if metadata.file_type().is_symlink() {
            return Err(InstallError::UnsafeRepositoryPath {
                path: path.as_str().to_owned(),
                message: "resolved parent is a symbolic link".to_owned(),
            });
        }
        if !metadata.is_dir() {
            return Err(InstallError::UnsafeRepositoryPath {
                path: path.as_str().to_owned(),
                message: "resolved parent is not a directory".to_owned(),
            });
        }
    }

    Ok(parent_absolute.to_path_buf())
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
