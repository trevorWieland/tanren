//! Shared path resolution guard for installer file operations.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use crate::install::error::InstallError;
use crate::install::manifest::RepoRelativePath;

/// Resolve a validated repo-relative path while rejecting symlink traversal.
pub(crate) fn resolve_repo_path(
    repository_root: &Path,
    path: &RepoRelativePath,
) -> Result<PathBuf, InstallError> {
    let mut absolute = repository_root.to_path_buf();
    let components = path.as_path().components().collect::<Vec<_>>();
    for (index, component) in components.iter().enumerate() {
        match component {
            Component::CurDir => {}
            Component::Normal(segment) => {
                absolute.push(segment);
                if !absolute.exists() {
                    continue;
                }

                let metadata =
                    fs::symlink_metadata(&absolute).map_err(|err| InstallError::ReadFailure {
                        path: path.as_str().to_owned(),
                        message: err.to_string(),
                    })?;

                if metadata.file_type().is_symlink() {
                    return Err(InstallError::UnsafeRepositoryPath {
                        path: path.as_str().to_owned(),
                        message: format!(
                            "resolved segment '{}' is a symbolic link",
                            segment.to_string_lossy()
                        ),
                    });
                }

                let is_last = index + 1 == components.len();
                if !is_last && !metadata.is_dir() {
                    return Err(InstallError::UnsafeRepositoryPath {
                        path: path.as_str().to_owned(),
                        message: format!(
                            "resolved segment '{}' is not a directory",
                            segment.to_string_lossy()
                        ),
                    });
                }
            }
            _ => {
                return Err(InstallError::InvalidRepoRelativePath {
                    path: path.as_str().to_owned(),
                });
            }
        }
    }
    Ok(absolute)
}

/// Revalidate that the destination path is not a symlink. Called immediately
/// before opening a staging temp file and immediately after creating parent
/// directories to close the TOCTOU window between path resolution and the
/// actual filesystem mutation.
pub(crate) fn revalidate_destination_symlink_status(
    path: &RepoRelativePath,
    absolute: &Path,
) -> Result<(), InstallError> {
    match fs::symlink_metadata(absolute) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(InstallError::UnsafeRepositoryPath {
                    path: path.as_str().to_owned(),
                    message: "resolved destination is a symbolic link".to_owned(),
                });
            }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(InstallError::ReadFailure {
                path: path.as_str().to_owned(),
                message: err.to_string(),
            });
        }
    }
    Ok(())
}

/// Create directory tree, routing the syscall through a single guarded helper.
pub(crate) fn guarded_create_dir_all(
    path: &RepoRelativePath,
    dir: &Path,
) -> Result<(), InstallError> {
    fs::create_dir_all(dir).map_err(|err| InstallError::CreateDirectoryFailure {
        path: path.as_str().to_owned(),
        message: err.to_string(),
    })
}

/// Atomically rename a file, routing the syscall through a single guarded helper.
pub(crate) fn guarded_rename(
    path: &RepoRelativePath,
    source: &Path,
    destination: &Path,
) -> Result<(), InstallError> {
    fs::rename(source, destination).map_err(|err| InstallError::WriteFailure {
        path: path.as_str().to_owned(),
        message: err.to_string(),
    })
}

/// Remove a file, ignoring `ErrorKind::NotFound`. Routes the syscall through a
/// single guarded helper and avoids the `Path::exists`-then-act TOCTOU race.
pub(crate) fn guarded_remove_file(path: &Path) {
    match fs::remove_file(path) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        _ => {}
    }
}

/// Outcome of [`guarded_try_create_new_file`].
pub(crate) enum CreateNewFileOutcome {
    /// File was created and opened for writing.
    Created(fs::File),
    /// A file already existed at the given path; caller may retry with a
    /// different name.
    AlreadyExists,
}

/// Open a new file for writing via `OpenOptions::create_new`, routing the
/// syscall through a single guarded helper. Returns a typed outcome so the
/// caller can distinguish the `AlreadyExists` retry case from hard errors.
pub(crate) fn guarded_try_create_new_file(
    path: &RepoRelativePath,
    file_path: &Path,
) -> Result<CreateNewFileOutcome, InstallError> {
    match fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(file_path)
    {
        Ok(file) => Ok(CreateNewFileOutcome::Created(file)),
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
            Ok(CreateNewFileOutcome::AlreadyExists)
        }
        Err(err) => Err(InstallError::WriteFailure {
            path: path.as_str().to_owned(),
            message: err.to_string(),
        }),
    }
}
