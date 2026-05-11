//! Shared path resolution guard for installer file operations.

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

use crate::install::error::InstallError;
use crate::install::manifest::RepoRelativePath;

/// Resolve a raw repo-relative path while rejecting symlink traversal.
pub(crate) fn resolve_repo_relative_path(
    repository_root: &Path,
    path: &str,
) -> Result<PathBuf, InstallError> {
    let path = RepoRelativePath::parse(path)?;
    resolve_repo_path(repository_root, &path)
}

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

// ---------------------------------------------------------------------------
// ParentHandle: open directory fd for race-resilient mutation
// ---------------------------------------------------------------------------
//
// The struct and methods are `pub` when the `test-hooks` feature is
// enabled (so the BDD testkit can re-export them) and `pub(crate)`
// otherwise (so the internal writer can use them without triggering the
// `unreachable_pub` lint).

/// Open handle to a parent directory for race-resilient mutation.
///
/// Created during the prepare phase by opening the parent directory and
/// recording its device/inode identity via [`std::fs::File::metadata`]
/// (which uses `fstat` on the held file descriptor). Immediately before
/// mutation, [`ParentHandle::verify`] re-reads the parent directory
/// path and cross-checks the path-resolved metadata against the open
/// file descriptor's live metadata. If the path now resolves to a
/// different directory (e.g. because a symlink was swapped between
/// prepare and commit), verification fails with
/// [`InstallError::ParentDirectoryChanged`].
///
/// The open file descriptor serves as the ground-truth reference: it
/// always refers to the directory that was approved during the prepare
/// phase, regardless of subsequent path-level changes (rename, symlink
/// swap, removal and recreation). The path-based stat in `verify`
/// detects when the path no longer leads to that same directory.
#[cfg(feature = "test-hooks")]
#[derive(Debug)]
pub struct ParentHandle {
    file: fs::File,
    device: u64,
    inode: u64,
}

#[cfg(not(feature = "test-hooks"))]
#[derive(Debug)]
pub(crate) struct ParentHandle {
    file: fs::File,
    device: u64,
    inode: u64,
}

impl ParentHandle {
    /// Open the parent of `absolute` as a directory file descriptor, and
    /// snapshot its dev/inode identity via `fstat`.
    #[cfg(feature = "test-hooks")]
    pub fn open(relative: &RepoRelativePath, absolute: &Path) -> Result<Self, InstallError> {
        Self::open_inner(relative, absolute)
    }

    #[cfg(not(feature = "test-hooks"))]
    pub(crate) fn open(relative: &RepoRelativePath, absolute: &Path) -> Result<Self, InstallError> {
        Self::open_inner(relative, absolute)
    }

    /// Verify the parent directory is still the same directory that was
    /// opened during [`ParentHandle::open`].
    ///
    /// Performs two checks:
    /// 1. **Fd consistency**: re-reads the held file descriptor's metadata
    ///    via `fstat` and confirms it matches the stored snapshot.
    /// 2. **Path-to-fd binding**: re-stats the parent path derived from
    ///    `absolute` and confirms it resolves to the same dev/inode as
    ///    the held file descriptor. This detects symlink swaps, directory
    ///    removals, or any path-level redirection between the prepare and
    ///    commit phases.
    ///
    /// Returns `Err(InstallError::ParentDirectoryChanged)` on any mismatch.
    #[cfg(feature = "test-hooks")]
    pub fn verify(&self, relative: &RepoRelativePath, absolute: &Path) -> Result<(), InstallError> {
        self.verify_inner(relative, absolute)
    }

    #[cfg(not(feature = "test-hooks"))]
    pub(crate) fn verify(
        &self,
        relative: &RepoRelativePath,
        absolute: &Path,
    ) -> Result<(), InstallError> {
        self.verify_inner(relative, absolute)
    }

    fn open_inner(relative: &RepoRelativePath, absolute: &Path) -> Result<Self, InstallError> {
        let Some(parent) = absolute.parent() else {
            return Err(InstallError::WriteFailure {
                path: relative.as_str().to_owned(),
                message: "destination path has no parent directory".to_owned(),
            });
        };
        let file = fs::File::open(parent).map_err(|err| InstallError::ReadFailure {
            path: relative.as_str().to_owned(),
            message: format!("failed opening parent directory: {err}"),
        })?;
        let metadata = file.metadata().map_err(|err| InstallError::ReadFailure {
            path: relative.as_str().to_owned(),
            message: format!("failed reading parent directory metadata: {err}"),
        })?;
        Ok(Self {
            file,
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }

    fn verify_inner(
        &self,
        relative: &RepoRelativePath,
        absolute: &Path,
    ) -> Result<(), InstallError> {
        let fd_metadata =
            self.file
                .metadata()
                .map_err(|err| InstallError::ParentDirectoryChanged {
                    path: relative.as_str().to_owned(),
                    message: format!("failed re-reading parent directory fd: {err}"),
                })?;
        if fd_metadata.dev() != self.device || fd_metadata.ino() != self.inode {
            return Err(InstallError::ParentDirectoryChanged {
                path: relative.as_str().to_owned(),
                message:
                    "parent directory file descriptor identity changed between resolution and commit"
                        .to_owned(),
            });
        }

        let Some(parent) = absolute.parent() else {
            return Err(InstallError::ParentDirectoryChanged {
                path: relative.as_str().to_owned(),
                message: "destination path has no parent directory".to_owned(),
            });
        };
        let path_metadata =
            fs::metadata(parent).map_err(|err| InstallError::ParentDirectoryChanged {
                path: relative.as_str().to_owned(),
                message: format!("failed re-reading parent directory path: {err}"),
            })?;
        if path_metadata.dev() != fd_metadata.dev() || path_metadata.ino() != fd_metadata.ino() {
            return Err(InstallError::ParentDirectoryChanged {
                path: relative.as_str().to_owned(),
                message: "parent directory path resolves to a different directory than the one opened during prepare".to_owned(),
            });
        }
        Ok(())
    }
}
