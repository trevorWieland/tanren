//! Shared path resolution guard for installer file operations.

use std::fs;
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
