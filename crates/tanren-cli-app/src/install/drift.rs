//! Read-only install drift reporting derived from install plans.

use crate::install::manifest::RepoRelativePath;
use crate::install::plan::{InstallPlan, PlannedWriteKind};

/// Per-path drift outcome for planned install reconciliation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InstallDriftKind {
    /// Asset is missing and would be created/restored by apply.
    Missing,
    /// Asset exists but differs from generated output and would be updated.
    Modified,
    /// Asset is stale and would be removed by apply.
    Stale,
    /// Asset differs but is intentionally preserved by policy.
    Accepted,
}

/// Typed, per-path install drift record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallDriftRecord {
    path: RepoRelativePath,
    kind: InstallDriftKind,
}

impl InstallDriftRecord {
    /// Repository-relative path for this drift outcome.
    #[must_use]
    pub fn path(&self) -> &RepoRelativePath {
        &self.path
    }

    /// Drift outcome category.
    #[must_use]
    pub const fn kind(&self) -> InstallDriftKind {
        self.kind
    }
}

/// Sorted read-only drift report derived from an install plan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstallDriftReport {
    entries: Vec<InstallDriftRecord>,
}

impl InstallDriftReport {
    /// Sorted drift outcomes for all paths relevant to this check.
    #[must_use]
    pub fn entries(&self) -> &[InstallDriftRecord] {
        &self.entries
    }

    /// True when at least one path requires create/restore/update/remove.
    #[must_use]
    pub fn has_drift(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.kind != InstallDriftKind::Accepted)
    }

    fn sort(&mut self) {
        self.entries.sort_by(|left, right| {
            left.path
                .as_str()
                .cmp(right.path.as_str())
                .then_with(|| left.kind.cmp(&right.kind))
        });
    }
}

/// Build a drift report from a validated install plan.
pub(super) fn build_install_drift_report(plan: &InstallPlan) -> InstallDriftReport {
    let mut entries =
        Vec::with_capacity(plan.writes().len() + plan.removals().len() + plan.preserved().len());

    for write in plan.writes() {
        let kind = match write.kind() {
            PlannedWriteKind::Created | PlannedWriteKind::Restored => InstallDriftKind::Missing,
            PlannedWriteKind::Updated => InstallDriftKind::Modified,
        };
        entries.push(InstallDriftRecord {
            path: write.path().clone(),
            kind,
        });
    }

    for removal in plan.removals() {
        entries.push(InstallDriftRecord {
            path: removal.path().clone(),
            kind: InstallDriftKind::Stale,
        });
    }

    for path in plan.preserved() {
        entries.push(InstallDriftRecord {
            path: path.clone(),
            kind: InstallDriftKind::Accepted,
        });
    }

    let mut report = InstallDriftReport { entries };
    report.sort();
    report
}
