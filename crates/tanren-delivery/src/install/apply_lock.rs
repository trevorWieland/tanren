//! Exclusive repo-local apply lock for serialized install/upgrade mutations.

use std::fs;
use std::path::{Path, PathBuf};

use crate::install::error::InstallError;

const APPLY_LOCK_RELATIVE_PATH: &str = ".tanren/.apply-lock";

/// RAII guard that holds an exclusive file lock for the duration of an apply.
///
/// The lock file lives at `<repo>/.tanren/.apply-lock` and is acquired via
/// `flock(2)`-style exclusive locking through the `fs2` crate. The lock is
/// released automatically when the guard is dropped.
#[derive(Debug)]
pub(super) struct ApplyLockGuard {
    _lock_file: fs::File,
    lock_path: PathBuf,
}

impl Drop for ApplyLockGuard {
    fn drop(&mut self) {
        cleanup_temporary_lock_file(&self.lock_path);
    }
}

/// Acquire an exclusive apply lock for the given repository root.
///
/// Creates the `.tanren/` directory if absent, then opens (or creates) the
/// lock file and acquires an exclusive lock. The lock is held until the
/// returned guard is dropped.
pub(super) fn acquire_apply_lock(repository_root: &Path) -> Result<ApplyLockGuard, InstallError> {
    let lock_dir = repository_root.join(".tanren");
    if !lock_dir.exists() {
        fs::create_dir_all(&lock_dir).map_err(|err| InstallError::CreateDirectoryFailure {
            path: ".tanren".to_owned(),
            message: err.to_string(),
        })?;
    }

    let lock_path = lock_dir.join(".apply-lock");
    let lock_file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|err| InstallError::WriteFailure {
            path: APPLY_LOCK_RELATIVE_PATH.to_owned(),
            message: err.to_string(),
        })?;

    fs2::FileExt::lock_exclusive(&lock_file).map_err(|err| InstallError::WriteFailure {
        path: APPLY_LOCK_RELATIVE_PATH.to_owned(),
        message: format!("failed to acquire exclusive apply lock: {err}"),
    })?;

    Ok(ApplyLockGuard {
        _lock_file: lock_file,
        lock_path,
    })
}

fn cleanup_temporary_lock_file(path: &Path) {
    if path.exists() {
        let _ = fs::remove_file(path);
    }
}
