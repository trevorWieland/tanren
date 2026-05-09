//! Filesystem writer for manifest-driven install plans.

use std::fs;
use std::path::Path;

use crate::install::error::InstallError;
use crate::install::manifest::RepoRelativePath;
use crate::install::plan::{InstallPlan, PlannedWriteKind};

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
    let mut report = InstallReport {
        preserved: plan.preserved.clone(),
        ..InstallReport::default()
    };

    for path in &plan.removals {
        remove_file(&plan.repository_root, path)?;
        report.removed.push(path.clone());
    }

    for write in &plan.writes {
        write_file(&plan.repository_root, &write.path, write.content.as_bytes())?;
        match write.kind {
            PlannedWriteKind::Created => report.created.push(write.path.clone()),
            PlannedWriteKind::Updated => report.updated.push(write.path.clone()),
            PlannedWriteKind::Restored => report.restored.push(write.path.clone()),
        }
    }

    write_manifest(plan, &mut report)?;
    report.sort_paths();
    Ok(report)
}

fn write_manifest(plan: &InstallPlan, report: &mut InstallReport) -> Result<(), InstallError> {
    let manifest_path = &plan.manifest_path;
    let absolute = plan.repository_root.join(manifest_path.as_path());
    let prior = fs::read(&absolute).ok();

    let payload =
        toml::to_string(&plan.manifest).map_err(|err| InstallError::InvalidInstallManifest {
            path: absolute.display().to_string(),
            message: err.to_string(),
        })?;

    write_file(&plan.repository_root, manifest_path, payload.as_bytes())?;

    match prior {
        None => report.created.push(manifest_path.clone()),
        Some(previous_payload) if previous_payload != payload.as_bytes() => {
            report.updated.push(manifest_path.clone());
        }
        Some(_) => {}
    }

    Ok(())
}

fn remove_file(repository_root: &Path, path: &RepoRelativePath) -> Result<(), InstallError> {
    let absolute = repository_root.join(path.as_path());
    fs::remove_file(&absolute).map_err(|err| InstallError::RemoveFailure {
        path: absolute.display().to_string(),
        message: err.to_string(),
    })
}

fn write_file(
    repository_root: &Path,
    path: &RepoRelativePath,
    content: &[u8],
) -> Result<(), InstallError> {
    let absolute = repository_root.join(path.as_path());
    if let Some(parent) = absolute.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent).map_err(|err| InstallError::CreateDirectoryFailure {
            path: parent.display().to_string(),
            message: err.to_string(),
        })?;
    }

    fs::write(&absolute, content).map_err(|err| InstallError::WriteFailure {
        path: absolute.display().to_string(),
        message: err.to_string(),
    })
}
