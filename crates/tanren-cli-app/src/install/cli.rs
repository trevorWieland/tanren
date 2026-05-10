//! CLI adapters for install-related `tanren-cli` commands.

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;

use crate::install::error::{InstallCommandError, InstallDriftCommandError};
use crate::install::manifest::RepoRelativePath;
use crate::install::{
    InstallDriftReport, InstallDriftStatus, InstallReport, apply_install, check_install_drift,
};

/// `tanren-cli install` arguments.
#[derive(Debug, Clone, Args)]
pub struct InstallCommand {
    /// Repository path to install into (defaults to current directory).
    #[arg(long, default_value = ".")]
    repo: PathBuf,
    /// Standards profile to install.
    #[arg(long)]
    profile: String,
    /// Comma-separated integration names (defaults to all).
    #[arg(long)]
    integrations: Option<String>,
}

impl InstallCommand {
    /// Validate install inputs, apply install, and emit a concise outcome report.
    pub fn run(&self) -> Result<(), InstallCommandError> {
        let report = apply_install(&self.repo, &self.profile, self.integrations.as_deref())
            .map_err(InstallCommandError::from)?;
        self.write_success_report(&report)
    }

    fn write_success_report(&self, report: &InstallReport) -> Result<(), InstallCommandError> {
        let repository = display_repository_argument(&self.repo);
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        writeln!(
            handle,
            "status=ok command=install repo={} created={} updated={} removed={} restored={} preserved={}",
            repository,
            report.created.len(),
            report.updated.len(),
            report.removed.len(),
            report.restored.len(),
            report.preserved.len(),
        )
        .map_err(|source| InstallCommandError::StdoutWriteFailure { source })?;
        writeln!(
            handle,
            "paths created=[{}] updated=[{}] removed=[{}] restored=[{}] preserved=[{}]",
            format_path_list(&report.created),
            format_path_list(&report.updated),
            format_path_list(&report.removed),
            format_path_list(&report.restored),
            format_path_list(&report.preserved),
        )
        .map_err(|source| InstallCommandError::StdoutWriteFailure { source })?;
        Ok(())
    }
}

/// `tanren-cli drift` arguments.
#[derive(Debug, Clone, Args)]
pub struct DriftCommand {
    /// Repository path to analyze (defaults to current directory).
    #[arg(long, default_value = ".")]
    repo: PathBuf,
    /// Standards profile to analyze.
    #[arg(long)]
    profile: String,
    /// Comma-separated integration names (defaults to all).
    #[arg(long)]
    integrations: Option<String>,
}

impl DriftCommand {
    /// Validate drift inputs, analyze install-managed paths, and emit a concise report.
    pub fn run(&self) -> Result<(), InstallDriftCommandError> {
        let report = check_install_drift(&self.repo, &self.profile, self.integrations.as_deref())
            .map_err(InstallDriftCommandError::from)?;
        self.write_report(&report)?;

        if report.has_drift() {
            return Err(InstallDriftCommandError::DriftDetected {
                drift_count: report.drift_count(),
            });
        }
        Ok(())
    }

    fn write_report(&self, report: &InstallDriftReport) -> Result<(), InstallDriftCommandError> {
        let summary = DriftSummary::from_report(report);
        let repository = display_repository_argument(&self.repo);
        let integrations = self.integrations.as_deref().unwrap_or("all");
        let status = if report.has_drift() { "drift" } else { "ok" };

        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        writeln!(
            handle,
            "status={} command=drift repo={} profile={} integrations={} clean={} changed_generated={} missing_generated={} missing_preserved={} accepted_preserved={} drift={}",
            status,
            repository,
            self.profile,
            integrations,
            summary.clean.len(),
            summary.changed_generated.len(),
            summary.missing_generated.len(),
            summary.missing_preserved.len(),
            summary.accepted_preserved.len(),
            report.drift_count(),
        )
        .map_err(|source| InstallDriftCommandError::StdoutWriteFailure { source })?;
        writeln!(
            handle,
            "paths clean=[{}] changed_generated=[{}] missing_generated=[{}] missing_preserved=[{}] accepted_preserved=[{}]",
            format_path_list(&summary.clean),
            format_path_list(&summary.changed_generated),
            format_path_list(&summary.missing_generated),
            format_path_list(&summary.missing_preserved),
            format_path_list(&summary.accepted_preserved),
        )
        .map_err(|source| InstallDriftCommandError::StdoutWriteFailure { source })?;
        Ok(())
    }
}

#[derive(Debug, Default)]
struct DriftSummary {
    clean: Vec<RepoRelativePath>,
    changed_generated: Vec<RepoRelativePath>,
    missing_generated: Vec<RepoRelativePath>,
    missing_preserved: Vec<RepoRelativePath>,
    accepted_preserved: Vec<RepoRelativePath>,
}

impl DriftSummary {
    fn from_report(report: &InstallDriftReport) -> Self {
        let mut summary = Self::default();
        for entry in report.entries() {
            let path = entry.path().clone();
            match entry.status() {
                InstallDriftStatus::Clean => summary.clean.push(path),
                InstallDriftStatus::ChangedGeneratedAsset => summary.changed_generated.push(path),
                InstallDriftStatus::MissingGeneratedAsset => summary.missing_generated.push(path),
                InstallDriftStatus::MissingPreservedStandard => {
                    summary.missing_preserved.push(path);
                }
                InstallDriftStatus::AcceptedPreservedEdit => summary.accepted_preserved.push(path),
            }
        }
        summary
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
