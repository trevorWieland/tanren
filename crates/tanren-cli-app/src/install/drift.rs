//! Read-only install drift analysis over the validated install plan.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::install::error::InstallDriftError;
use crate::install::manifest::{PreservationPolicy, RepoRelativePath};
use crate::install::plan::PlannedWriteKind;
use crate::install::plan_install;

/// Typed drift status for one planned install asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstallDriftStatus {
    /// Repository file matches the current install projection.
    Clean,
    /// Tanren-owned generated file exists but content differs from projection.
    ChangedGeneratedAsset,
    /// Tanren-owned generated file is missing from the repository.
    MissingGeneratedAsset,
    /// Preserved standards profile file is missing from the repository.
    MissingPreservedStandard,
    /// Preserved standards profile file was edited and accepted as user-owned.
    AcceptedPreservedEdit,
}

impl InstallDriftStatus {
    /// Whether this status should count as drift.
    #[must_use]
    pub(crate) const fn is_drift(self) -> bool {
        matches!(
            self,
            Self::ChangedGeneratedAsset
                | Self::MissingGeneratedAsset
                | Self::MissingPreservedStandard
        )
    }
}

/// Drift status for one install-managed repository path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstallDriftEntry {
    path: RepoRelativePath,
    status: InstallDriftStatus,
}

impl InstallDriftEntry {
    /// Repo-relative path for this drift status row.
    #[must_use]
    pub(crate) fn path(&self) -> &RepoRelativePath {
        &self.path
    }

    /// Typed drift status for this row.
    #[must_use]
    pub(crate) const fn status(&self) -> InstallDriftStatus {
        self.status
    }
}

/// Read-only drift report for install-managed repository assets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstallDriftReport {
    entries: Vec<InstallDriftEntry>,
}

impl InstallDriftReport {
    /// One row per install-managed asset in deterministic path order.
    #[must_use]
    pub(crate) fn entries(&self) -> &[InstallDriftEntry] {
        &self.entries
    }

    /// Number of entries currently classified as drift.
    #[must_use]
    pub(crate) fn drift_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.status().is_drift())
            .count()
    }

    /// Whether drift exists for any install-managed path.
    #[must_use]
    pub(crate) fn has_drift(&self) -> bool {
        self.drift_count() > 0
    }
}

/// Validate install inputs and analyze drift without mutating repository files.
pub(super) fn check_install_drift(
    repository: &Path,
    profile: &str,
    integration_selection: Option<&str>,
) -> Result<InstallDriftReport, InstallDriftError> {
    let plan = plan_install(repository, profile, integration_selection)?;

    let writes = plan
        .writes()
        .iter()
        .map(|write| (write.path().as_str(), write.kind()))
        .collect::<BTreeMap<_, _>>();
    let preserved = plan
        .preserved()
        .iter()
        .map(RepoRelativePath::as_str)
        .collect::<BTreeSet<_>>();

    let mut entries = Vec::with_capacity(plan.manifest().entries.len());
    for manifest_entry in &plan.manifest().entries {
        let path = manifest_entry.path.as_str();
        let status = classify_status(
            manifest_entry.preservation,
            writes.get(path).copied(),
            preserved.contains(path),
        );
        entries.push(InstallDriftEntry {
            path: manifest_entry.path.clone(),
            status,
        });
    }

    entries.sort_by(|left, right| left.path().as_str().cmp(right.path().as_str()));
    Ok(InstallDriftReport { entries })
}

fn classify_status(
    preservation: PreservationPolicy,
    write_kind: Option<PlannedWriteKind>,
    was_preserved: bool,
) -> InstallDriftStatus {
    match preservation {
        PreservationPolicy::ReplaceGenerated => match write_kind {
            Some(PlannedWriteKind::Updated) => InstallDriftStatus::ChangedGeneratedAsset,
            Some(PlannedWriteKind::Created | PlannedWriteKind::Restored) => {
                InstallDriftStatus::MissingGeneratedAsset
            }
            None => InstallDriftStatus::Clean,
        },
        PreservationPolicy::PreserveUserEdits => match write_kind {
            Some(PlannedWriteKind::Created | PlannedWriteKind::Restored) => {
                InstallDriftStatus::MissingPreservedStandard
            }
            Some(PlannedWriteKind::Updated) => InstallDriftStatus::AcceptedPreservedEdit,
            None if was_preserved => InstallDriftStatus::AcceptedPreservedEdit,
            None => InstallDriftStatus::Clean,
        },
    }
}
