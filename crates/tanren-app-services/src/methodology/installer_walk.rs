use std::path::{Path, PathBuf};

use super::errors::{MethodologyError, MethodologyResult};

pub(super) fn collect_walkable_files(
    root: &Path,
    resolved_root: &Path,
) -> MethodologyResult<Vec<PathBuf>> {
    let root_meta = match std::fs::symlink_metadata(root) {
        Ok(meta) => meta,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(MethodologyError::Io {
                path: root.to_path_buf(),
                source,
            });
        }
    };
    if root_meta.file_type().is_symlink() {
        return Err(MethodologyError::Validation(format!(
            "refusing destructive traversal of symlink root `{}`",
            root.display()
        )));
    }
    if !root_meta.is_dir() {
        return Ok(Vec::new());
    }

    let mut stack = vec![root.to_path_buf()];
    let mut out = Vec::new();
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|source| MethodologyError::Io {
            path: dir.clone(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| MethodologyError::Io {
                path: dir.clone(),
                source,
            })?;
            let path = entry.path();
            let metadata =
                std::fs::symlink_metadata(&path).map_err(|source| MethodologyError::Io {
                    path: path.clone(),
                    source,
                })?;
            let file_type = metadata.file_type();
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            if file_type.is_file() || file_type.is_symlink() {
                // Skip temporary install files from prior in-flight runs.
                if path.extension().is_some_and(|e| e == "tanren-install-tmp") {
                    continue;
                }
                validate_discovered_path_is_within_root(&path, resolved_root)?;
                out.push(path);
            }
        }
    }
    Ok(out)
}

fn validate_discovered_path_is_within_root(
    found: &Path,
    resolved_root: &Path,
) -> MethodologyResult<()> {
    let resolved_found = std::fs::canonicalize(found).map_err(|source| MethodologyError::Io {
        path: found.to_path_buf(),
        source,
    })?;
    if resolved_found.starts_with(resolved_root) {
        return Ok(());
    }
    Err(MethodologyError::Validation(format!(
        "refusing destructive traversal: discovered path `{}` escapes root `{}`",
        found.display(),
        resolved_root.display()
    )))
}
