//! CLI adapter for `tanren-cli install`.

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;

use crate::install::error::InstallCommandError;
use crate::install::manifest::RepoRelativePath;
use crate::install::{
    InstallDriftKind, InstallDriftReport, InstallReport, apply_install, check_install_drift,
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
    /// Check install drift without writing repository files.
    #[arg(long)]
    check: bool,
}

impl InstallCommand {
    /// Validate install inputs, then apply install writes or run read-only drift
    /// checks and emit a concise deterministic report.
    pub fn run(&self) -> Result<(), InstallCommandError> {
        if self.check {
            return self.run_check();
        }

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

    fn run_check(&self) -> Result<(), InstallCommandError> {
        let report = check_install_drift(&self.repo, &self.profile, self.integrations.as_deref())
            .map_err(InstallCommandError::from)?;
        self.write_check_report(&report)?;
        if report.has_drift() {
            return Err(InstallCommandError::DriftDetected);
        }
        Ok(())
    }

    fn write_check_report(&self, report: &InstallDriftReport) -> Result<(), InstallCommandError> {
        let repository = display_repository_argument(&self.repo);
        let (modified, missing, stale, accepted) = classify_drift_paths(report);
        let status = if report.has_drift() { "drift" } else { "ok" };
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        writeln!(
            handle,
            "status={} command=install-check repo={} modified={} missing={} stale={} accepted={}",
            status,
            repository,
            modified.len(),
            missing.len(),
            stale.len(),
            accepted.len(),
        )
        .map_err(|source| InstallCommandError::StdoutWriteFailure { source })?;
        writeln!(
            handle,
            "paths modified=[{}] missing=[{}] stale=[{}] accepted=[{}]",
            format_path_list(&modified),
            format_path_list(&missing),
            format_path_list(&stale),
            format_path_list(&accepted),
        )
        .map_err(|source| InstallCommandError::StdoutWriteFailure { source })?;
        Ok(())
    }
}

fn format_path_list(paths: &[RepoRelativePath]) -> String {
    paths
        .iter()
        .map(RepoRelativePath::as_str)
        .collect::<Vec<_>>()
        .join(",")
}

fn classify_drift_paths(
    report: &InstallDriftReport,
) -> (
    Vec<RepoRelativePath>,
    Vec<RepoRelativePath>,
    Vec<RepoRelativePath>,
    Vec<RepoRelativePath>,
) {
    let mut modified = Vec::new();
    let mut missing = Vec::new();
    let mut stale = Vec::new();
    let mut accepted = Vec::new();

    for entry in report.entries() {
        match entry.kind() {
            InstallDriftKind::Modified => modified.push(entry.path().clone()),
            InstallDriftKind::Missing => missing.push(entry.path().clone()),
            InstallDriftKind::Stale => stale.push(entry.path().clone()),
            InstallDriftKind::Accepted => accepted.push(entry.path().clone()),
        }
    }

    (modified, missing, stale, accepted)
}

fn display_repository_argument(path: &Path) -> String {
    if path.is_absolute() {
        "<redacted-absolute-path>".to_owned()
    } else {
        path.display().to_string()
    }
}
