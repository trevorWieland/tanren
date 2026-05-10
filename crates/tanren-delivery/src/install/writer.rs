//! Filesystem writer for manifest-driven install plans.

use crate::install::error::InstallError;
use crate::install::manifest::RepoRelativePath;
use crate::install::plan::{InstallPlan, PlannedWriteKind};
use crate::install::writer_tx::{
    ManifestChange, cleanup_rollback_scratch_files, cleanup_staged_payloads,
    commit_staged_replacement, prepare_apply, remove_prepared_file, resolve_apply_failure,
};

/// Install apply report grouped by observable outcome.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstallReport {
    pub created: Vec<RepoRelativePath>,
    pub updated: Vec<RepoRelativePath>,
    pub removed: Vec<RepoRelativePath>,
    pub restored: Vec<RepoRelativePath>,
    pub preserved: Vec<RepoRelativePath>,
}

impl InstallReport {
    fn sort_paths(&mut self) {
        self.created.sort();
        self.updated.sort();
        self.removed.sort();
        self.restored.sort();
        self.preserved.sort();
    }
}

/// Apply a previously validated install plan.
pub(super) fn apply_install_plan(plan: &InstallPlan) -> Result<InstallReport, InstallError> {
    let prepared = prepare_apply(plan)?;
    let mut report = InstallReport {
        preserved: plan.preserved().to_vec(),
        ..InstallReport::default()
    };
    let mut changed_paths = Vec::new();

    let apply_result: Result<(), InstallError> = (|| {
        for removal in &prepared.removals {
            remove_prepared_file(plan, &removal.path, &removal.absolute)?;
            report.removed.push(removal.path.clone());
            changed_paths.push(removal.path.clone());
        }

        for write in &prepared.writes {
            commit_staged_replacement(plan, &write.staged)?;
            match write.kind {
                PlannedWriteKind::Created => report.created.push(write.staged.path.clone()),
                PlannedWriteKind::Updated => report.updated.push(write.staged.path.clone()),
                PlannedWriteKind::Restored => report.restored.push(write.staged.path.clone()),
            }
            changed_paths.push(write.staged.path.clone());
        }

        if let Some(manifest) = &prepared.manifest.staged {
            commit_staged_replacement(plan, manifest)?;
            match prepared.manifest.change {
                ManifestChange::Created => report.created.push(manifest.path.clone()),
                ManifestChange::Updated => report.updated.push(manifest.path.clone()),
                ManifestChange::Unchanged => {}
            }
            changed_paths.push(manifest.path.clone());
        }

        Ok(())
    })();

    cleanup_staged_payloads(&prepared);

    if let Err(error) = apply_result {
        let rollback_error =
            resolve_apply_failure(plan, &prepared.rollback_records, &changed_paths, error);
        cleanup_rollback_scratch_files(&prepared.rollback_records);
        return Err(rollback_error);
    }

    cleanup_rollback_scratch_files(&prepared.rollback_records);
    report.sort_paths();
    Ok(report)
}
