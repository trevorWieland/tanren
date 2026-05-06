use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Subcommand;
use tanren_configuration_secrets::{ProjectConfig, load_standards};

#[derive(Debug, Subcommand)]
pub(crate) enum StandardsAction {
    Inspect {
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
    },
}

pub(crate) fn dispatch(action: StandardsAction) -> Result<()> {
    match action {
        StandardsAction::Inspect { project_dir } => {
            let config = ProjectConfig::load(&project_dir)?;
            let root = config.standards_root(&project_dir);
            let bundle = load_standards(&root)?;
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            writeln!(handle, "standards_root={}", bundle.root.display())
                .context("write standards root")?;
            writeln!(handle, "count={}", bundle.standards.len())
                .context("write standards count")?;
            for standard in &bundle.standards {
                writeln!(
                    handle,
                    "name={} category={} importance={}",
                    standard.name, standard.category, standard.importance
                )
                .context("write standard entry")?;
            }
            Ok(())
        }
    }
}
