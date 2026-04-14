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
use tanren_contract::{ContractError, ErrorResponse};
use tanren_observability::{ObservabilityError, init_tracing};
use uuid::Uuid;

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
            let error_response = into_error_response(err);
            if let Ok(json) = serde_json::to_string_pretty(&error_response) {
                let _ = writeln!(std::io::stderr(), "{json}");
            }
            std::process::ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<()> {
    let cli = Cli::try_parse().map_err(|err| anyhow::Error::new(clap_error_to_response(&err)))?;

    match init_tracing(&cli.log_level) {
        Ok(()) | Err(ObservabilityError::AlreadyInitialized) => {}
        Err(ObservabilityError::FilterParse(reason)) => {
            return Err(anyhow::Error::new(ErrorResponse::from(
                ContractError::InvalidField {
                    field: "log_level".to_owned(),
                    reason,
                },
            )));
        }
    }

    let service = tanren_app_services::compose::build_dispatch_service(&cli.database_url).await?;

    match cli.command {
        Commands::Dispatch(cmd) => commands::dispatch::handle(cmd, &service).await,
    }
}

fn into_error_response(err: anyhow::Error) -> ErrorResponse {
    match err.downcast::<ErrorResponse>() {
        Ok(er) => er,
        Err(other) => {
            let correlation_id = Uuid::now_v7();
            tracing::error!(
                %correlation_id,
                error = %other,
                ?other,
                "unhandled cli failure mapped to internal error response",
            );
            ErrorResponse {
                code: "internal".to_owned(),
                message: "internal error".to_owned(),
                details: Some(serde_json::json!({
                    "correlation_id": correlation_id.to_string(),
                })),
            }
        }
    }
}

fn clap_error_to_response(err: &clap::Error) -> ErrorResponse {
    ErrorResponse::from(ContractError::InvalidField {
        field: "cli_args".to_owned(),
        reason: err.to_string(),
    })
}
