use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::install::manifest::RepoRelativePath;

use super::InstallProofError;

/// Snapshot of repository files and bytes used by uninstall proof helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallProofUninstallRepositorySnapshotBaseline {
    files: BTreeMap<RepoRelativePath, Vec<u8>>,
}

/// Capture a repository snapshot for uninstall preview/no-op proof assertions.
pub fn capture_uninstall_repository_snapshot(
    repository_root: &Path,
) -> Result<InstallProofUninstallRepositorySnapshotBaseline, InstallProofError> {
    let mut files = BTreeMap::new();
    collect_snapshot_files(repository_root, repository_root, &mut files)?;
    Ok(InstallProofUninstallRepositorySnapshotBaseline { files })
}

/// Assert the repository snapshot matches a previously recorded baseline snapshot.
pub fn assert_uninstall_repository_snapshot_matches_baseline(
    repository_root: &Path,
    baseline: &InstallProofUninstallRepositorySnapshotBaseline,
) -> Result<(), InstallProofError> {
    let observed = capture_uninstall_repository_snapshot(repository_root)?;
    if observed != *baseline {
        return Err(InstallProofError::ExpectedRepositorySnapshotToMatchBaseline);
    }
    Ok(())
}

/// Assert uninstall preview leaves the repository snapshot unchanged.
pub fn assert_uninstall_preview_keeps_repository_snapshot_unchanged(
    repository_root: &Path,
    baseline: &InstallProofUninstallRepositorySnapshotBaseline,
) -> Result<(), InstallProofError> {
    assert_uninstall_repository_snapshot_matches_baseline(repository_root, baseline)
}

/// Assert uninstall no-install runs leave the repository snapshot unchanged.
pub fn assert_uninstall_no_install_keeps_repository_snapshot_unchanged(
    repository_root: &Path,
    baseline: &InstallProofUninstallRepositorySnapshotBaseline,
) -> Result<(), InstallProofError> {
    assert_uninstall_repository_snapshot_matches_baseline(repository_root, baseline)
}

fn collect_snapshot_files(
    root: &Path,
    cursor: &Path,
    out: &mut BTreeMap<RepoRelativePath, Vec<u8>>,
) -> Result<(), InstallProofError> {
    let entries = fs::read_dir(cursor).map_err(|source| InstallProofError::ReadDirectory {
        path: cursor.to_path_buf(),
        action: "capture repository snapshot",
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| InstallProofError::ReadDirectoryEntry {
            path: cursor.to_path_buf(),
            action: "capture repository snapshot entries",
            source,
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| InstallProofError::InspectFileType {
                path: path.clone(),
                action: "capture repository snapshot entry type",
                source,
            })?;
        if file_type.is_dir() {
            collect_snapshot_files(root, &path, out)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let relative = path.strip_prefix(root).map_err(|source| {
            InstallProofError::SnapshotPathOutsideRoot {
                path: path.clone(),
                source,
            }
        })?;
        let normalized = relative.to_string_lossy().replace('\\', "/");
        let relative_path = RepoRelativePath::parse(&normalized).map_err(|source| {
            InstallProofError::SnapshotPathContractRejected {
                path: normalized.clone(),
                source,
            }
        })?;
        let bytes = fs::read(&path).map_err(|source| InstallProofError::ReadFile {
            path,
            action: "read repository snapshot file bytes",
            source,
        })?;
        out.insert(relative_path, bytes);
    }
    Ok(())
}
