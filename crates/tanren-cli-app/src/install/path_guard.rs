//! Shared path resolution guard for installer file operations.

use std::fs;
use std::io::ErrorKind;
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

/// Canonicalize and validate a repository root directory.
pub(crate) fn validate_repository_root(repository: &Path) -> Result<PathBuf, InstallError> {
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

/// Display a repository path, redacting absolute paths.
pub(crate) fn display_repository_argument(path: &Path) -> String {
    if path.is_absolute() {
        "<redacted-absolute-path>".to_owned()
    } else {
        path.display().to_string()
    }
}

/// Map an IO error kind to a stable redacted string.
pub(crate) fn redacted_io_error_kind(kind: ErrorKind) -> &'static str {
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
