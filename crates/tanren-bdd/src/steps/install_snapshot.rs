use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::install_error::{InstallStepError, InstallStepResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepositorySnapshot {
    files: BTreeMap<String, Vec<u8>>,
}

impl RepositorySnapshot {
    pub(crate) fn capture(root: &Path) -> InstallStepResult<Self> {
        let mut files = BTreeMap::new();
        collect_files(root, root, &mut files)?;
        Ok(Self { files })
    }

    #[must_use]
    pub(crate) fn changed_paths(&self, other: &Self) -> Vec<String> {
        let mut changed = Vec::new();

        for (path, bytes) in &self.files {
            match other.files.get(path) {
                Some(other_bytes) if other_bytes == bytes => {}
                Some(_) | None => changed.push(path.clone()),
            }
        }
        for path in other.files.keys() {
            if !self.files.contains_key(path) {
                changed.push(path.clone());
            }
        }

        changed.sort_unstable();
        changed.dedup();
        changed
    }
}

fn collect_files(
    root: &Path,
    cursor: &Path,
    out: &mut BTreeMap<String, Vec<u8>>,
) -> InstallStepResult<()> {
    let entries = fs::read_dir(cursor).map_err(|source| InstallStepError::ReadDirectory {
        path: cursor.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| InstallStepError::ReadDirectoryEntry {
            path: cursor.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| InstallStepError::InspectFileType {
                path: path.clone(),
                source,
            })?;
        if file_type.is_dir() {
            collect_files(root, &path, out)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let relative =
            path.strip_prefix(root)
                .map_err(|source| InstallStepError::PathOutsideRoot {
                    path: path.clone(),
                    root: root.to_path_buf(),
                    source,
                })?;
        let relative = relative.to_string_lossy().replace('\\', "/");
        let bytes = fs::read(&path).map_err(|source| InstallStepError::ReadFile {
            path: path.clone(),
            action: "read repository fixture file bytes",
            source,
        })?;
        out.insert(relative, bytes);
    }
    Ok(())
}
