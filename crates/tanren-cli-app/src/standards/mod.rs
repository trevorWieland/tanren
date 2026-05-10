//! CLI standards inspection command.

use std::io;
use std::path::PathBuf;

use clap::{Args, Subcommand};

mod config;
mod error;
mod report;
mod scan;

pub(crate) use error::StandardsCommandError;

/// `tanren-cli standards` command arguments.
#[derive(Debug, Clone, Args)]
pub(crate) struct StandardsCommand {
    #[command(subcommand)]
    action: StandardsAction,
}

impl StandardsCommand {
    /// Dispatch standards subcommands.
    pub(crate) fn run(&self) -> Result<(), StandardsCommandError> {
        match &self.action {
            StandardsAction::Inspect(command) => command.run(),
        }
    }
}

/// Supported `tanren-cli standards` subcommands.
#[derive(Debug, Clone, Subcommand)]
enum StandardsAction {
    /// Inspect standards from the repository's configured standards root.
    Inspect(StandardsInspectCommand),
}

/// `tanren-cli standards inspect` command arguments.
#[derive(Debug, Clone, Args)]
struct StandardsInspectCommand {
    /// Repository path to inspect.
    #[arg(long)]
    repo: PathBuf,
}

impl StandardsInspectCommand {
    fn run(&self) -> Result<(), StandardsCommandError> {
        let success_report = inspect_standards(&self.repo)?;
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        report::write_success_report(&mut handle, &success_report)
    }
}

fn inspect_standards(
    repository: &std::path::Path,
) -> Result<report::StandardsInspectSuccessReport, StandardsCommandError> {
    let targets = config::resolve_inspection_targets(repository)?;
    let scan_summary =
        scan::scan_standards(targets.repository_root(), targets.standards_root_path())?;
    report::build_inspect_success_report(repository, &targets, scan_summary)
}
