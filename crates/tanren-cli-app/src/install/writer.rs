//! Filesystem writer for manifest-driven install plans.

use crate::install::error::InstallError;
use crate::install::manifest::RepoRelativePath;
use crate::install::plan::{InstallPlan, PlannedWriteKind};
use crate::install::writer_tx::{
    cleanup_staged_payloads, commit_staged_replacement, prepare_apply, resolve_apply_failure,
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
pub fn apply_install_plan(plan: &InstallPlan) -> Result<InstallReport, InstallError> {
    let prepared = prepare_apply(plan)?;
    let mut report = InstallReport {
        preserved: plan.preserved().to_vec(),
        ..InstallReport::default()
    };
    let mut changed_paths = Vec::new();

    let apply_result: Result<(), InstallError> = (|| {
        for removal in &prepared.removals {
            std::fs::remove_file(&removal.absolute).map_err(|err| InstallError::RemoveFailure {
                path: removal.absolute.display().to_string(),
                message: err.to_string(),
            })?;
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

        commit_staged_replacement(plan, &prepared.manifest.staged)?;
        if prepared.manifest.prior.is_none() {
            report.created.push(prepared.manifest.staged.path.clone());
        } else if prepared.manifest.prior.as_deref() != Some(prepared.manifest.payload.as_slice()) {
            report.updated.push(prepared.manifest.staged.path.clone());
        }
        changed_paths.push(prepared.manifest.staged.path.clone());

        Ok(())
    })();

    cleanup_staged_payloads(&prepared);

    if let Err(error) = apply_result {
        return Err(resolve_apply_failure(
            plan,
            &prepared.rollback_records,
            &changed_paths,
            error,
        ));
    }

    report.sort_paths();
    Ok(report)
}
