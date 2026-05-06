mod apply;
mod assets;
mod manifest;
mod model;
mod profile;
mod render;

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
    let effective = input.effective_integrations();
    let resolved = profile::resolve(&input.profile.to_string());
    let commands = assets::command_files();
    let rendered = render::render_all_integrations(&effective);
    let result = apply::apply(
        &input.repo,
        &input.profile.to_string(),
        &commands,
        &rendered,
        &resolved.standards,
    )?;

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(
        handle,
        "install: profile={} commands={} integrations={} standards={}",
        result.manifest.profile,
        commands.len(),
        effective.len(),
        result.manifest.standards.len(),
    )
    .context("write install result")?;

    let outcome = &result.outcome;
    if !outcome.created.is_empty() {
        writeln!(handle, "  created: {}", outcome.created.join(", "))
            .context("write created summary")?;
    }
    if !outcome.updated.is_empty() {
        writeln!(handle, "  updated: {}", outcome.updated.join(", "))
            .context("write updated summary")?;
    }
    if !outcome.removed.is_empty() {
        writeln!(handle, "  removed: {}", outcome.removed.join(", "))
            .context("write removed summary")?;
    }
    if !outcome.restored.is_empty() {
        writeln!(handle, "  restored: {}", outcome.restored.join(", "))
            .context("write restored summary")?;
    }
    if !outcome.preserved.is_empty() {
        writeln!(handle, "  preserved: {}", outcome.preserved.join(", "))
            .context("write preserved summary")?;
    }
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
