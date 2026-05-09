//! CLI adapter for `tanren-cli install`.

use std::io::Write;
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use clap::Args;

use crate::install::error::InstallError;
use crate::install::manifest::RepoRelativePath;
use crate::install::{InstallReport, apply_install};

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
    pub fn run(&self) -> Result<()> {
        let report = apply_install(&self.repo, &self.profile, self.integrations.as_deref())
            .map_err(|err| classify_install_error(&err))?;
        self.write_success_report(&report)
    }

    fn write_success_report(&self, report: &InstallReport) -> Result<()> {
        let repository = self
            .repo
            .canonicalize()
            .unwrap_or_else(|_| self.repo.clone());
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        writeln!(
            handle,
            "status=ok command=install repo={} created={} updated={} removed={} restored={} preserved={}",
            repository.display(),
            report.created.len(),
            report.updated.len(),
            report.removed.len(),
            report.restored.len(),
            report.preserved.len(),
        )?;
        writeln!(
            handle,
            "paths created=[{}] updated=[{}] removed=[{}] restored=[{}] preserved=[{}]",
            format_path_list(&report.created),
            format_path_list(&report.updated),
            format_path_list(&report.removed),
            format_path_list(&report.restored),
            format_path_list(&report.preserved),
        )?;
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

fn classify_install_error(err: &InstallError) -> anyhow::Error {
    match err {
        InstallError::UnsupportedProfile { .. }
        | InstallError::UnsupportedIntegration { .. }
        | InstallError::EmptyIntegrationSelection
        | InstallError::InvalidRepositoryPath { .. }
        | InstallError::UnsafeRepositoryPath { .. }
        | InstallError::RepositoryPathNotDirectory { .. } => {
            anyhow!("error: validation_failed — {err}")
        }
        _ => anyhow!("error: install_failed — {err}"),
    }
}
