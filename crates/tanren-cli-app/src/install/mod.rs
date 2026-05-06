mod model;

use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;

use model::{InstallInput, InstallValidationError};

#[derive(Debug, Args)]
pub(crate) struct InstallArgs {
    #[arg(
        long,
        help = "Standards profile to install (default, react-ts-pnpm, rust-cargo)"
    )]
    pub(crate) profile: String,

    #[arg(
        long,
        default_value = ".",
        help = "Repository path. Defaults to the current directory."
    )]
    pub(crate) repo: PathBuf,

    #[arg(
        long,
        value_delimiter = ',',
        help = "Comma-delimited agent integrations (claude, codex, opencode)."
    )]
    pub(crate) integrations: Option<Vec<String>>,
}

pub(crate) fn run_install(args: InstallArgs) -> Result<()> {
    let input = validate(args)?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    let integration_count = input.integrations.len();
    writeln!(
        handle,
        "install: validated profile={} repo={} integrations={integration_count}",
        input.profile,
        input.repo.display(),
    )
    .context("write install result")?;
    Ok(())
}

fn validate(args: InstallArgs) -> Result<InstallInput> {
    let profile = model::ProfileName::parse(&args.profile)?;

    let repo = if args.repo.is_absolute() {
        args.repo
    } else {
        std::env::current_dir()
            .context("resolve current directory")?
            .join(&args.repo)
    };

    if !repo.exists() {
        return Err(InstallValidationError::RepoNotFound(repo).into());
    }
    if !repo.is_dir() {
        return Err(InstallValidationError::NotADirectory(repo).into());
    }

    let integrations = match args.integrations {
        Some(names) => names
            .iter()
            .map(|name| model::IntegrationName::parse(name).map_err(Into::into))
            .collect::<Result<Vec<_>>>()?,
        None => Vec::new(),
    };

    Ok(InstallInput {
        repo,
        profile,
        integrations,
    })
}
