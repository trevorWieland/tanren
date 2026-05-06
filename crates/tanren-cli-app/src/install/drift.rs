use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use super::assets::{AssetKind, asset_catalog};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DriftState {
    Clean,
    Missing,
    Modified,
    Accepted,
}

#[derive(Debug, Serialize)]
pub(crate) struct DriftEntry {
    pub(crate) path: String,
    pub(crate) state: DriftState,
}

#[derive(Debug, Serialize)]
pub(crate) struct DriftReport {
    pub(crate) has_drift: bool,
    pub(crate) entries: Vec<DriftEntry>,
}

pub(crate) fn check_drift(repo: &Path) -> Result<DriftReport> {
    let catalog = asset_catalog();
    let mut entries = Vec::with_capacity(catalog.len());
    let mut has_drift = false;

    for spec in &catalog {
        let full_path = repo.join(spec.rel_path);
        let state = match spec.kind {
            AssetKind::Generated => check_generated(&full_path, spec.expected_content)?,
            AssetKind::PreservedStandard => check_preserved(&full_path)?,
        };
        if !matches!(state, DriftState::Clean | DriftState::Accepted) {
            has_drift = true;
        }
        entries.push(DriftEntry {
            path: spec.rel_path.to_owned(),
            state,
        });
    }

    Ok(DriftReport { has_drift, entries })
}

fn check_generated(path: &Path, expected_content: Option<&str>) -> Result<DriftState> {
    let expected = expected_content.expect("generated assets must carry expected content");
    match std::fs::read_to_string(path) {
        Ok(actual) if actual == expected => Ok(DriftState::Clean),
        Ok(_) => Ok(DriftState::Modified),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(DriftState::Missing),
        Err(e) => Err(e).with_context(|| format!("read generated asset {}", path.display())),
    }
}

fn check_preserved(path: &Path) -> Result<DriftState> {
    match std::fs::metadata(path) {
        Ok(_) => Ok(DriftState::Accepted),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(DriftState::Missing),
        Err(e) => Err(e).with_context(|| format!("check preserved standard {}", path.display())),
    }
}
