//! CLI adapter for `tanren-cli install`.

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
            report.created_count(),
            report.updated_count(),
            report.removed_count(),
            report.restored_count(),
            report.preserved_count(),
        )
        .map_err(|source| InstallCommandError::StdoutWriteFailure { source })?;
        write!(handle, "paths created=[",)
            .map_err(|source| InstallCommandError::StdoutWriteFailure { source })?;
        emit_path_list(&report.created, &mut handle)?;
        write!(handle, "] updated=[",)
            .map_err(|source| InstallCommandError::StdoutWriteFailure { source })?;
        emit_path_list(&report.updated, &mut handle)?;
        write!(handle, "] removed=[",)
            .map_err(|source| InstallCommandError::StdoutWriteFailure { source })?;
        emit_path_list(&report.removed, &mut handle)?;
        write!(handle, "] restored=[",)
            .map_err(|source| InstallCommandError::StdoutWriteFailure { source })?;
        emit_path_list(&report.restored, &mut handle)?;
        write!(handle, "] preserved=[",)
            .map_err(|source| InstallCommandError::StdoutWriteFailure { source })?;
        emit_path_list(&report.preserved, &mut handle)?;
        writeln!(handle, "]",)
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
            report.clean_count(),
            report.changed_generated_count(),
            report.missing_generated_count(),
            report.missing_preserved_count(),
            report.accepted_preserved_count(),
            report.drift_count(),
        )
        .map_err(|source| InstallDriftCommandError::StdoutWriteFailure { source })?;

        // Stream per-detail records in a single pass over entries — no
        // intermediate DriftSummary Vec<RepoRelativePath> allocations.
        for entry in report.entries() {
            let detail_status = match entry.status() {
                InstallDriftStatus::ChangedGeneratedAsset => "changed_generated",
                InstallDriftStatus::MissingGeneratedAsset => "missing_generated",
                InstallDriftStatus::MissingPreservedStandard => "missing_preserved",
                InstallDriftStatus::AcceptedPreservedEdit => "accepted_preserved",
                InstallDriftStatus::Clean => "clean",
            };
            writeln!(
                handle,
                "detail status={} path={}",
                detail_status,
                entry.path().as_str(),
            )
            .map_err(|source| InstallDriftCommandError::StdoutWriteFailure { source })?;
        }
        Ok(())
    }
}

fn emit_path_list(
    paths: &[RepoRelativePath],
    handle: &mut std::io::StdoutLock<'_>,
) -> Result<(), InstallCommandError> {
    let mut first = true;
    for path in paths {
        if first {
            first = false;
        } else {
            write!(handle, ",")
                .map_err(|source| InstallCommandError::StdoutWriteFailure { source })?;
        }
        write!(handle, "{}", path.as_str())
            .map_err(|source| InstallCommandError::StdoutWriteFailure { source })?;
    }
    Ok(())
}

fn display_repository_argument(path: &Path) -> String {
    if path.is_absolute() {
        "<redacted-absolute-path>".to_owned()
    } else {
        path.display().to_string()
    }
}
