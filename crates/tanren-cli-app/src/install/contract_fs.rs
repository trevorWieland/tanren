use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::install::InstallIntegration;
use crate::install::catalog::generated_integration_destination_roots;

use super::InstallProofError;

pub(super) fn assert_unselected_integration_roots_are_empty(
    repository_root: &Path,
    selected: &BTreeSet<InstallIntegration>,
) -> Result<(), InstallProofError> {
    for integration in InstallIntegration::all() {
        if selected.contains(&integration) {
            continue;
        }

        let roots = generated_integration_destination_roots(&BTreeSet::from([integration]));
        for root in roots {
            let root_path = repository_root.join(root);
            if has_any_files(&root_path)? {
                return Err(InstallProofError::ExpectedFileToBeAbsent { path: root_path });
            }
        }
    }

    Ok(())
}

pub(super) fn assert_file_exists(
    repository_root: &Path,
    relative_path: &str,
) -> Result<(), InstallProofError> {
    let absolute = repository_root.join(relative_path);
    if !absolute.exists() {
        return Err(InstallProofError::ExpectedFileToExist { path: absolute });
    }
    Ok(())
}

pub(super) fn read_to_string_with_context(
    path: &Path,
    action: &'static str,
) -> Result<String, InstallProofError> {
    fs::read_to_string(path).map_err(|source| InstallProofError::ReadFile {
        path: path.to_path_buf(),
        action,
        source,
    })
}

#[cfg(feature = "test-hooks")]
pub(super) fn workspace_root() -> Result<PathBuf, InstallProofError> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|source| InstallProofError::CanonicalizeWorkspaceRoot {
            action: "resolving workspace root",
            source,
        })
}

fn has_any_files(path: &Path) -> Result<bool, InstallProofError> {
    if !path.exists() {
        return Ok(false);
    }

    let entries = fs::read_dir(path).map_err(|source| InstallProofError::ReadDirectory {
        path: path.to_path_buf(),
        action: "inspect unselected integration root",
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| InstallProofError::ReadDirectoryEntry {
            path: path.to_path_buf(),
            action: "inspect unselected integration root entries",
            source,
        })?;
        let entry_path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| InstallProofError::InspectFileType {
                path: entry_path.clone(),
                action: "inspect unselected integration root entry type",
                source,
            })?;

        if file_type.is_file() {
            return Ok(true);
        }
        if file_type.is_dir() && has_any_files(&entry_path)? {
            return Ok(true);
        }
    }

    Ok(false)
}
