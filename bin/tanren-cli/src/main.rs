//! Tanren CLI — the composition root for command-line operation.
//!
//! Parses args, initializes tracing, and delegates to the app-services
//! composition root for wiring, then dispatches to the appropriate
//! subcommand handler.
//!
//! All failures — including startup errors — produce deterministic JSON
//! on stderr and a non-zero exit code.

mod commands;

use std::io::Write as _;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tanren_contract::ErrorResponse;
use tanren_observability::init_tracing;

/// Tanren — agent orchestration control plane.
#[derive(Debug, Parser)]
#[command(name = "tanren", version, about)]
struct Cli {
    /// Database URL (`SQLite` or Postgres).
    #[arg(long, default_value = "sqlite:tanren.db", global = true)]
    database_url: String,

    /// Log level filter (e.g. "info", "debug").
    #[arg(long, default_value = "warn", global = true)]
    log_level: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Manage dispatches.
    #[command(subcommand)]
    Dispatch(commands::dispatch::DispatchCommand),
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    match run().await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            let error_response = match err.downcast::<ErrorResponse>() {
                Ok(er) => er,
                Err(other) => ErrorResponse {
                    code: "internal".to_owned(),
                    message: other.to_string(),
                    details: None,
                },
            };
            if let Ok(json) = serde_json::to_string_pretty(&error_response) {
                let _ = writeln!(std::io::stderr(), "{json}");
            }
            std::process::ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing — ignore double-init errors (e.g. in tests)
    let _ = init_tracing(&cli.log_level);

    let service = tanren_app_services::compose::build_dispatch_service(&cli.database_url).await?;

    match cli.command {
        Commands::Dispatch(cmd) => commands::dispatch::handle(cmd, &service).await,
    }
}
