//! CLI adapters for install-related `tanren-cli` commands.

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;
use tanren_contract::cli_output::{
    CommandName, DriftDetailStatus, DriftStatus, InstallStatus, OutputKey, RecordKind,
    write_bracket_list, write_count_field, write_field, write_typed_field,
};

use crate::install::drift::InstallDriftStatus;
use crate::install::error::{InstallCommandError, InstallDriftCommandError};
use crate::install::manifest::RepoRelativePath;
use crate::install::{
    InstallDriftReport, InstallReport, InstallSelection, apply_install, check_install_drift,
};

/// `tanren-cli install` arguments.
#[derive(Debug, Clone, Args)]
pub(crate) struct InstallCommand {
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
    pub(crate) fn run(&self) -> Result<(), InstallCommandError> {
        let selection = self.parse_selection().map_err(InstallCommandError::from)?;
        let report = apply_install(&self.repo, selection.profile(), selection.integrations())
            .map_err(InstallCommandError::from)?;
        self.write_success_report(&report)
    }

    fn parse_selection(&self) -> Result<InstallSelection, crate::install::InstallError> {
        Ok(InstallSelection::parse(
            &self.profile,
            self.integrations.as_deref(),
        )?)
    }

    fn write_success_report(&self, report: &InstallReport) -> Result<(), InstallCommandError> {
        let repository = display_repository_argument(&self.repo);
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();

        let mut line = String::new();
        line.push_str(RecordKind::Summary.as_str());
        write_typed_field(&mut line, OutputKey::Status, InstallStatus::Ok).expect("fmt");
        write_typed_field(&mut line, OutputKey::Command, CommandName::Install).expect("fmt");
        write_field(&mut line, OutputKey::Repo, &repository).expect("fmt");
        write_count_field(&mut line, OutputKey::Created, report.created.len()).expect("fmt");
        write_count_field(&mut line, OutputKey::Updated, report.updated.len()).expect("fmt");
        write_count_field(&mut line, OutputKey::Removed, report.removed.len()).expect("fmt");
        write_count_field(&mut line, OutputKey::Restored, report.restored.len()).expect("fmt");
        write_count_field(&mut line, OutputKey::Preserved, report.preserved.len()).expect("fmt");
        writeln!(handle, "{line}")
            .map_err(|source| InstallCommandError::StdoutWriteFailure { source })?;

        let mut line = String::new();
        line.push_str(RecordKind::Paths.as_str());
        write_bracket_list(&mut line, OutputKey::Created, &path_strs(&report.created))
            .expect("fmt");
        write_bracket_list(&mut line, OutputKey::Updated, &path_strs(&report.updated))
            .expect("fmt");
        write_bracket_list(&mut line, OutputKey::Removed, &path_strs(&report.removed))
            .expect("fmt");
        write_bracket_list(&mut line, OutputKey::Restored, &path_strs(&report.restored))
            .expect("fmt");
        write_bracket_list(
            &mut line,
            OutputKey::Preserved,
            &path_strs(&report.preserved),
        )
        .expect("fmt");
        writeln!(handle, "{line}")
            .map_err(|source| InstallCommandError::StdoutWriteFailure { source })?;

        Ok(())
    }
}

/// `tanren-cli drift` arguments.
#[derive(Debug, Clone, Args)]
pub(crate) struct DriftCommand {
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
    pub(crate) fn run(&self) -> Result<(), InstallDriftCommandError> {
        let selection = self
            .parse_selection()
            .map_err(crate::install::InstallDriftError::from)
            .map_err(InstallDriftCommandError::from)?;
        let report = check_install_drift(&self.repo, selection.profile(), selection.integrations())
            .map_err(InstallDriftCommandError::from)?;
        self.write_report(&report, &selection)?;

        if report.has_drift() {
            return Err(InstallDriftCommandError::DriftDetected {
                drift_count: report.drift_count(),
            });
        }
        Ok(())
    }

    fn parse_selection(&self) -> Result<InstallSelection, crate::install::InstallError> {
        Ok(InstallSelection::parse(
            &self.profile,
            self.integrations.as_deref(),
        )?)
    }

    fn write_report(
        &self,
        report: &InstallDriftReport,
        selection: &InstallSelection,
    ) -> Result<(), InstallDriftCommandError> {
        let summary = DriftSummary::from_report(report);
        let repository = display_repository_argument(&self.repo);
        let status = if report.has_drift() {
            DriftStatus::Drift
        } else {
            DriftStatus::Ok
        };

        let stdout = std::io::stdout();
        let mut handle = stdout.lock();

        let mut line = String::new();
        line.push_str(RecordKind::Summary.as_str());
        write_typed_field(&mut line, OutputKey::Status, status).expect("fmt");
        write_typed_field(&mut line, OutputKey::Command, CommandName::Drift).expect("fmt");
        write_field(&mut line, OutputKey::Repo, &repository).expect("fmt");
        write_field(&mut line, OutputKey::Profile, selection.profile().as_str()).expect("fmt");
        write_field(
            &mut line,
            OutputKey::Integrations,
            &selection.integrations_csv(),
        )
        .expect("fmt");
        write_count_field(&mut line, OutputKey::Clean, summary.clean.len()).expect("fmt");
        write_count_field(
            &mut line,
            OutputKey::ChangedGenerated,
            summary.changed_generated.len(),
        )
        .expect("fmt");
        write_count_field(
            &mut line,
            OutputKey::MissingGenerated,
            summary.missing_generated.len(),
        )
        .expect("fmt");
        write_count_field(
            &mut line,
            OutputKey::MissingPreserved,
            summary.missing_preserved.len(),
        )
        .expect("fmt");
        write_count_field(
            &mut line,
            OutputKey::AcceptedPreserved,
            summary.accepted_preserved.len(),
        )
        .expect("fmt");
        write_count_field(&mut line, OutputKey::Drift, report.drift_count()).expect("fmt");
        writeln!(handle, "{line}")
            .map_err(|source| InstallDriftCommandError::StdoutWriteFailure { source })?;

        for path in &summary.changed_generated {
            write_detail_line(
                &mut handle,
                DriftDetailStatus::ChangedGenerated,
                path.as_str(),
            )?;
        }
        for path in &summary.missing_generated {
            write_detail_line(
                &mut handle,
                DriftDetailStatus::MissingGenerated,
                path.as_str(),
            )?;
        }
        for path in &summary.missing_preserved {
            write_detail_line(
                &mut handle,
                DriftDetailStatus::MissingPreserved,
                path.as_str(),
            )?;
        }
        for path in &summary.accepted_preserved {
            write_detail_line(
                &mut handle,
                DriftDetailStatus::AcceptedPreserved,
                path.as_str(),
            )?;
        }
        for path in &summary.clean {
            write_detail_line(&mut handle, DriftDetailStatus::Clean, path.as_str())?;
        }
        Ok(())
    }
}

fn write_detail_line(
    handle: &mut std::io::StdoutLock<'_>,
    status: DriftDetailStatus,
    path: &str,
) -> Result<(), InstallDriftCommandError> {
    let mut line = String::new();
    line.push_str(RecordKind::Detail.as_str());
    write_typed_field(&mut line, OutputKey::Status, status).expect("fmt");
    write_field(&mut line, OutputKey::Path, path).expect("fmt");
    writeln!(handle, "{line}")
        .map_err(|source| InstallDriftCommandError::StdoutWriteFailure { source })
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

fn path_strs(paths: &[RepoRelativePath]) -> Vec<&str> {
    paths.iter().map(RepoRelativePath::as_str).collect()
}

fn display_repository_argument(path: &Path) -> String {
    if path.is_absolute() {
        "<redacted-absolute-path>".to_owned()
    } else {
        path.display().to_string()
    }
}
