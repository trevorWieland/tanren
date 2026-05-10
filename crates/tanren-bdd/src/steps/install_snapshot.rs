use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use tanren_testkit::sha256_hex_string;

use super::install_error::{InstallStepError, InstallStepResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepositorySnapshot {
    entries: BTreeMap<String, RepositorySnapshotEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RepositorySnapshotEntry {
    Directory,
    File { sha256: String },
    Symlink { target: String },
}

impl RepositorySnapshot {
    pub(crate) fn capture(root: &Path) -> InstallStepResult<Self> {
        let mut entries = BTreeMap::new();
        collect_entries(root, root, &mut entries)?;
        Ok(Self { entries })
    }
}

fn collect_entries(
    root: &Path,
    cursor: &Path,
    out: &mut BTreeMap<String, RepositorySnapshotEntry>,
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
        let relative =
            path.strip_prefix(root)
                .map_err(|source| InstallStepError::PathOutsideRoot {
                    path: path.clone(),
                    root: root.to_path_buf(),
                    source,
                })?;
        let relative = relative.to_string_lossy().replace('\\', "/");
        if file_type.is_dir() {
            out.insert(relative, RepositorySnapshotEntry::Directory);
            collect_entries(root, &path, out)?;
            continue;
        }
        if file_type.is_symlink() {
            let target = fs::read_link(&path).map_err(|source| InstallStepError::Io {
                path: path.clone(),
                action: "read repository fixture symlink target",
                source,
            })?;
            out.insert(
                relative,
                RepositorySnapshotEntry::Symlink {
                    target: target.to_string_lossy().replace('\\', "/"),
                },
            );
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let bytes = fs::read(&path).map_err(|source| InstallStepError::ReadFile {
            path: path.clone(),
            action: "read repository fixture file bytes",
            source,
        })?;
        out.insert(
            relative,
            RepositorySnapshotEntry::File {
                sha256: sha256_hex_string(&bytes),
            },
        );
    }
    Ok(())
}
