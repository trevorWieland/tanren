//! Read-only install drift analysis over the validated install plan.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::install::error::InstallDriftError;
use crate::install::manifest::{PreservationPolicy, RepoRelativePath};
use crate::install::plan::PlannedWriteKind;
use crate::install::plan_install;

/// Typed drift status for one planned install asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallDriftStatus {
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
    pub const fn is_drift(self) -> bool {
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
pub struct InstallDriftEntry {
    path: RepoRelativePath,
    status: InstallDriftStatus,
}

impl InstallDriftEntry {
    /// Repo-relative path for this drift status row.
    #[must_use]
    pub fn path(&self) -> &RepoRelativePath {
        &self.path
    }

    /// Typed drift status for this row.
    #[must_use]
    pub const fn status(&self) -> InstallDriftStatus {
        self.status
    }
}

/// Precomputed drift summary counts — O(1) accessors with no per-call iteration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct DriftCounts {
    clean: usize,
    changed_generated: usize,
    missing_generated: usize,
    missing_preserved: usize,
    accepted_preserved: usize,
    drift: usize,
}

/// Read-only drift report for install-managed repository assets.
///
/// Category counts are precomputed once at construction time so
/// `has_drift`, `drift_count`, and per-status count accessors are O(1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallDriftReport {
    entries: Vec<InstallDriftEntry>,
    counts: DriftCounts,
}

impl InstallDriftReport {
    /// One row per install-managed asset in deterministic path order.
    #[must_use]
    pub fn entries(&self) -> &[InstallDriftEntry] {
        &self.entries
    }

    /// Number of entries with `Clean` status.
    #[must_use]
    pub fn clean_count(&self) -> usize {
        self.counts.clean
    }

    /// Number of entries with `ChangedGeneratedAsset` status.
    #[must_use]
    pub fn changed_generated_count(&self) -> usize {
        self.counts.changed_generated
    }

    /// Number of entries with `MissingGeneratedAsset` status.
    #[must_use]
    pub fn missing_generated_count(&self) -> usize {
        self.counts.missing_generated
    }

    /// Number of entries with `MissingPreservedStandard` status.
    #[must_use]
    pub fn missing_preserved_count(&self) -> usize {
        self.counts.missing_preserved
    }

    /// Number of entries with `AcceptedPreservedEdit` status.
    #[must_use]
    pub fn accepted_preserved_count(&self) -> usize {
        self.counts.accepted_preserved
    }

    /// Number of entries currently classified as drift.
    #[must_use]
    pub fn drift_count(&self) -> usize {
        self.counts.drift
    }

    /// Whether drift exists for any install-managed path.
    #[must_use]
    pub fn has_drift(&self) -> bool {
        self.counts.drift > 0
    }
}

/// Validate install inputs and analyze drift without mutating repository files.
pub(super) fn check_install_drift(
    repository: &Path,
    profile: &str,
    integration_selection: Option<&str>,
) -> Result<InstallDriftReport, InstallDriftError> {
    let plan = plan_install(repository, profile, integration_selection)?;

    let writes: BTreeMap<&RepoRelativePath, PlannedWriteKind> = plan
        .writes()
        .iter()
        .map(|write| (write.path(), write.kind()))
        .collect();
    let preserved: BTreeSet<&RepoRelativePath> = plan.preserved().iter().collect();

    let mut entries = Vec::with_capacity(plan.manifest().entries.len());
    let mut counts = DriftCounts::default();

    for manifest_entry in &plan.manifest().entries {
        let status = classify_status(
            manifest_entry.preservation,
            writes.get(&manifest_entry.path).copied(),
            preserved.contains(&manifest_entry.path),
        );
        if status.is_drift() {
            counts.drift += 1;
        }
        match status {
            InstallDriftStatus::Clean => counts.clean += 1,
            InstallDriftStatus::ChangedGeneratedAsset => counts.changed_generated += 1,
            InstallDriftStatus::MissingGeneratedAsset => counts.missing_generated += 1,
            InstallDriftStatus::MissingPreservedStandard => counts.missing_preserved += 1,
            InstallDriftStatus::AcceptedPreservedEdit => counts.accepted_preserved += 1,
        }
        entries.push(InstallDriftEntry {
            path: manifest_entry.path.clone(),
            status,
        });
    }

    entries.sort_by(|left, right| left.path().as_str().cmp(right.path().as_str()));
    Ok(InstallDriftReport { entries, counts })
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
