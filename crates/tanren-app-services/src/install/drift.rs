use std::fs;
use std::path::Path;

use tanren_contract::{
    DriftPolicy, InstallDriftAssetKind, InstallDriftEntry, InstallDriftState, PreservationPolicy,
};

use super::{PRESERVED_INPUTS, PROJECTION_MANIFEST};
use crate::AppServiceError;

#[derive(Debug)]
pub struct DriftEvalResult {
    pub has_drift: bool,
    pub entries: Vec<InstallDriftEntry>,
    pub drift_count: usize,
    pub missing_count: usize,
    pub accepted_count: usize,
    pub matches_count: usize,
}

pub fn evaluate_drift(
    repo_path: &Path,
    drift_policy: DriftPolicy,
    preservation_policy: PreservationPolicy,
) -> Result<DriftEvalResult, AppServiceError> {
    let span = tracing::info_span!(
        "drift_evaluate",
        drift_policy = ?drift_policy,
        preservation_policy = ?preservation_policy,
    );
    let _enter = span.enter();

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
    let mut drift_count = 0usize;
    let mut missing_count = 0usize;
    let mut accepted_count = 0usize;
    let mut matches_count = 0usize;

    for entry in PROJECTION_MANIFEST {
        let full_path = repo_path.join(entry.rel_path);
        let state = classify_generated(&full_path, entry);

        match state {
            InstallDriftState::Drifted => {
                has_drift = true;
                drift_count += 1;
            }
            InstallDriftState::Missing => {
                has_drift = true;
                missing_count += 1;
            }
            InstallDriftState::Accepted => {
                accepted_count += 1;
            }
            InstallDriftState::Matches => {
                matches_count += 1;
            }
        }

        entries.push(InstallDriftEntry {
            relative_path: entry.rel_path.to_owned(),
            asset_kind: entry.kind,
            state,
        });
    }

    if drift_policy == DriftPolicy::AllAssets {
        for entry in PRESERVED_INPUTS {
            let full_path = repo_path.join(entry.rel_path);
            let state = classify_preserved(&full_path, preservation_policy);

            match state {
                InstallDriftState::Drifted => {
                    has_drift = true;
                    drift_count += 1;
                }
                InstallDriftState::Missing => {
                    has_drift = true;
                    missing_count += 1;
                }
                InstallDriftState::Accepted => {
                    accepted_count += 1;
                }
                InstallDriftState::Matches => {
                    matches_count += 1;
                }
            }

            entries.push(InstallDriftEntry {
                relative_path: entry.rel_path.to_owned(),
                asset_kind: InstallDriftAssetKind::PreservedStandard,
                state,
            });
        }
    }

    entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    tracing::info!(
        asset_count = entries.len(),
        drift_count,
        missing_count,
        accepted_count,
        matches_count,
        has_drift,
        "drift evaluation complete"
    );

    Ok(DriftEvalResult {
        has_drift,
        entries,
        drift_count,
        missing_count,
        accepted_count,
        matches_count,
    })
}

fn classify_generated(path: &Path, entry: &super::ProjectionEntry) -> InstallDriftState {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return InstallDriftState::Missing;
    };

    if !meta.is_file() {
        return InstallDriftState::Drifted;
    }

    let Some(expected) = entry.expected_content else {
        return InstallDriftState::Matches;
    };

    match fs::read(path) {
        Ok(bytes) if bytes == expected.as_bytes() => InstallDriftState::Matches,
        _ => InstallDriftState::Drifted,
    }
}

fn classify_preserved(path: &Path, _preservation_policy: PreservationPolicy) -> InstallDriftState {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return InstallDriftState::Missing;
    };

    if !meta.is_file() {
        return InstallDriftState::Missing;
    }

    InstallDriftState::Accepted
}
