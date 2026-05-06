use std::fs;
use std::path::Path;

use tanren_contract::{
    DriftPolicy, InstallDriftAssetKind, InstallDriftEntry, InstallDriftState, PreservationPolicy,
};

use super::{EntryDriftPolicy, PROJECTION_MANIFEST};
use crate::AppServiceError;

pub(crate) struct DriftEvalResult {
    pub has_drift: bool,
    pub entries: Vec<InstallDriftEntry>,
}

pub(crate) fn evaluate_drift(
    repo_path: &Path,
    drift_policy: DriftPolicy,
    preservation_policy: PreservationPolicy,
) -> Result<DriftEvalResult, AppServiceError> {
    let repo_meta = fs::symlink_metadata(repo_path).map_err(|e| {
        AppServiceError::InvalidInput(format!(
            "repository path inaccessible: {}: {e}",
            repo_path.display()
        ))
    })?;

    if !repo_meta.is_dir() {
        return Err(AppServiceError::InvalidInput(format!(
            "repository path is not a directory: {}",
            repo_path.display()
        )));
    }

    let mut entries = Vec::new();
    let mut has_drift = false;

    for entry in PROJECTION_MANIFEST {
        if drift_policy == DriftPolicy::GeneratedOnly
            && entry.kind == InstallDriftAssetKind::PreservedStandard
        {
            continue;
        }

        let full_path = repo_path.join(entry.rel_path);
        let state = classify_entry(&full_path, entry, preservation_policy);

        if matches!(
            state,
            InstallDriftState::Drifted | InstallDriftState::Missing
        ) {
            has_drift = true;
        }

        entries.push(InstallDriftEntry {
            relative_path: entry.rel_path.to_owned(),
            asset_kind: entry.kind,
            state,
        });
    }

    entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    Ok(DriftEvalResult { has_drift, entries })
}

fn classify_entry(
    path: &Path,
    entry: &super::ProjectionEntry,
    preservation_policy: PreservationPolicy,
) -> InstallDriftState {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return InstallDriftState::Missing;
    };

    if !meta.is_file() {
        return InstallDriftState::Drifted;
    }

    let strict_preserved = entry.kind == InstallDriftAssetKind::PreservedStandard
        && preservation_policy == PreservationPolicy::Strict;

    if entry.drift_policy == EntryDriftPolicy::PresenceOnly && !strict_preserved {
        return InstallDriftState::Accepted;
    }

    let Some(expected) = entry.expected_content else {
        return InstallDriftState::Matches;
    };

    match fs::read(path) {
        Ok(bytes) if bytes == expected.as_bytes() => InstallDriftState::Matches,
        _ => InstallDriftState::Drifted,
    }
}
