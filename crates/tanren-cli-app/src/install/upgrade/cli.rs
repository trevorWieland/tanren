//! CLI adapter for `tanren-cli upgrade`.

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;

use crate::install::error::UpgradeCommandError;
use crate::install::manifest::RepoRelativePath;
use crate::install::upgrade::{
    UpgradePlanOutcome, UpgradePreviewReport, apply_upgrade, plan_upgrade,
};
use crate::install::writer::InstallReport;

/// `tanren-cli upgrade` arguments.
#[derive(Debug, Clone, Args)]
pub struct UpgradeCommand {
    /// Repository path to upgrade (defaults to current directory).
    #[arg(long, default_value = ".")]
    repo: PathBuf,
    /// Apply the previewed upgrade plan.
    #[arg(long)]
    confirm: bool,
}

impl UpgradeCommand {
    /// Preview or apply a manifest-aware upgrade and emit a concise outcome report.
    pub fn run(&self) -> Result<(), UpgradeCommandError> {
        let outcome = plan_upgrade(&self.repo).map_err(UpgradeCommandError::from)?;
        match outcome {
            UpgradePlanOutcome::NothingToUpgrade(noop) => {
                self.write_noop_report(noop.manifest_path())
            }
            UpgradePlanOutcome::Planned(plan) => {
                let preview = UpgradePreviewReport::from_plan(&plan);
                self.write_preview_report(&preview)?;
                if !self.confirm {
                    return Err(UpgradeCommandError::ConfirmationRequired);
                }
                let report = apply_upgrade(&plan).map_err(UpgradeCommandError::from)?;
                self.write_apply_report(&report)
            }
        }
    }

    fn write_noop_report(
        &self,
        manifest_path: &RepoRelativePath,
    ) -> Result<(), UpgradeCommandError> {
        let repository = display_repository_argument(&self.repo);
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        writeln!(
            handle,
            "status=ok command=upgrade repo={} outcome=nothing_to_upgrade manifest={}",
            repository,
            manifest_path.as_str(),
        )
        .map_err(|source| UpgradeCommandError::StdoutWriteFailure { source })
    }

    fn write_preview_report(
        &self,
        report: &UpgradePreviewReport,
    ) -> Result<(), UpgradeCommandError> {
        let repository = display_repository_argument(&self.repo);
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        writeln!(
            handle,
            "status=preview command=upgrade repo={} created={} updated={} removed={} restored={} preserved={} concerns={}",
            repository,
            report.created.len(),
            report.updated.len(),
            report.removed.len(),
            report.restored.len(),
            report.preserved.len(),
            report.concerns.len(),
        )
        .map_err(|source| UpgradeCommandError::StdoutWriteFailure { source })?;
        writeln!(
            handle,
            "paths created=[{}] updated=[{}] removed=[{}] restored=[{}] preserved=[{}] concerns=[{}]",
            format_path_list(&report.created),
            format_path_list(&report.updated),
            format_path_list(&report.removed),
            format_path_list(&report.restored),
            format_path_list(&report.preserved),
            report.concerns.join(","),
        )
        .map_err(|source| UpgradeCommandError::StdoutWriteFailure { source })
    }

    fn write_apply_report(&self, report: &InstallReport) -> Result<(), UpgradeCommandError> {
        let repository = display_repository_argument(&self.repo);
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        writeln!(
            handle,
            "status=ok command=upgrade repo={} created={} updated={} removed={} restored={} preserved={}",
            repository,
            report.created.len(),
            report.updated.len(),
            report.removed.len(),
            report.restored.len(),
            report.preserved.len(),
        )
        .map_err(|source| UpgradeCommandError::StdoutWriteFailure { source })?;
        writeln!(
            handle,
            "paths created=[{}] updated=[{}] removed=[{}] restored=[{}] preserved=[{}]",
            format_path_list(&report.created),
            format_path_list(&report.updated),
            format_path_list(&report.removed),
            format_path_list(&report.restored),
            format_path_list(&report.preserved),
        )
        .map_err(|source| UpgradeCommandError::StdoutWriteFailure { source })
    }
}

fn format_path_list(paths: &[RepoRelativePath]) -> String {
    paths
        .iter()
        .map(RepoRelativePath::as_str)
        .collect::<Vec<_>>()
        .join(",")
}

fn display_repository_argument(path: &Path) -> String {
    if path.is_absolute() {
        "<redacted-absolute-path>".to_owned()
    } else {
        path.display().to_string()
    }
}
