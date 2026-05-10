//! Preview report for manifest-aware upgrades.

use crate::install::manifest::RepoRelativePath;
use crate::install::upgrade::{
    MigrationConcern, UpgradePlan, UpgradePlannedWrite, UpgradePlannedWriteKind,
};

/// Human-readable preview grouped by observable outcome.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpgradePreviewReport {
    pub created: Vec<RepoRelativePath>,
    pub updated: Vec<RepoRelativePath>,
    pub removed: Vec<RepoRelativePath>,
    pub restored: Vec<RepoRelativePath>,
    pub preserved: Vec<RepoRelativePath>,
    pub concerns: Vec<String>,
}

impl UpgradePreviewReport {
    /// Build a preview report from a planned upgrade without mutating files.
    #[must_use]
    pub fn from_plan(plan: &UpgradePlan) -> Self {
        let mut report = Self {
            removed: plan
                .removals()
                .iter()
                .map(|removal| removal.path().clone())
                .collect(),
            preserved: plan.preserved().to_vec(),
            concerns: plan
                .migration_concerns()
                .iter()
                .map(format_concern)
                .collect(),
            ..Self::default()
        };

        for write in plan.writes() {
            report.push_write(write);
        }
        report.sort_paths();
        report
    }

    fn push_write(&mut self, write: &UpgradePlannedWrite) {
        match write.kind() {
            UpgradePlannedWriteKind::Created => self.created.push(write.path().clone()),
            UpgradePlannedWriteKind::Updated => self.updated.push(write.path().clone()),
            UpgradePlannedWriteKind::Restored => self.restored.push(write.path().clone()),
        }
    }

    fn sort_paths(&mut self) {
        self.created.sort();
        self.updated.sort();
        self.removed.sort();
        self.restored.sort();
        self.preserved.sort();
        self.concerns.sort();
    }
}

fn format_concern(concern: &MigrationConcern) -> String {
    match concern {
        MigrationConcern::CatalogRevisionChanged { from, to } => {
            format!("catalog_revision_changed:{from:?}->{to:?}")
        }
        MigrationConcern::PreservedUserEdit { path } => {
            format!("preserved_user_edit:{}", path.as_str())
        }
        MigrationConcern::DriftedStaleGeneratedAsset { path } => {
            format!("drifted_stale_generated_asset:{}", path.as_str())
        }
    }
}
