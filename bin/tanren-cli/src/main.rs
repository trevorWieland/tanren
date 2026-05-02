//! Tanren scriptable command-line client.
//!
//! F-0001 ships only `--version`, `health` (cross-interface liveness probe),
//! and `migrate up` (applies pending database migrations via the
//! tanren-store runner). Behavior commands arrive with R-* slices that route
//! through `tanren-app-services`.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::io::Write;
use tanren_app_services::Handlers;

#[derive(Debug, Parser)]
#[command(
    name = "tanren-cli",
    version,
    about = "Tanren scriptable command-line client"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print a liveness report. Mirrors the api `/health` endpoint.
    Health,
    /// Database migration commands.
    Migrate {
        #[command(subcommand)]
        action: MigrateAction,
    },
}

#[derive(Debug, Subcommand)]
enum MigrateAction {
    /// Apply all pending migrations.
    Up {
        /// Database URL (defaults to `$DATABASE_URL`).
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        None | Some(Command::Health) => print_health(),
        Some(Command::Migrate {
            action: MigrateAction::Up { database_url },
        }) => run_migrate_up(&database_url),
    }
}

fn print_health() -> Result<()> {
    let report = Handlers::new().health(env!("CARGO_PKG_VERSION"));
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(
        handle,
        "status={status} version={version} contract_version={contract}",
        status = report.status,
        version = report.version,
        contract = report.contract_version.value(),
    )
    .context("write health report to stdout")?;
    Ok(())
}

fn run_migrate_up(database_url: &str) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;
    runtime.block_on(async {
        Handlers::new()
            .migrate(database_url)
            .await
            .context("apply pending migrations")
    })?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(handle, "migrations: applied").context("write migrate report to stdout")?;
    Ok(())
}
