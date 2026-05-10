//! CLI adapter for `tanren-cli install`.

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;

use super::error::InstallCommandError;

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
        let report = tanren_delivery::install::apply_install(
            &self.repo,
            &self.profile,
            self.integrations.as_deref(),
        )
        .map_err(InstallCommandError::from)?;
        self.write_success_report(&report)
    }

    fn write_success_report(
        &self,
        report: &tanren_delivery::install::InstallReport,
    ) -> Result<(), InstallCommandError> {
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

fn format_path_list(paths: &[tanren_delivery::install::RepoRelativePath]) -> String {
    paths
        .iter()
        .map(tanren_delivery::install::RepoRelativePath::as_str)
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
