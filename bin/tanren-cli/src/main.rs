//! Tanren CLI — the composition root for command-line operation.
//!
//! Parses args, initializes tracing, and delegates to the app-services
//! composition root for wiring, then dispatches to the appropriate
//! subcommand handler.
//!
//! All failures — including startup errors — produce deterministic JSON
//! on stderr and a non-zero exit code.

mod commands;

use std::io::Read as _;
use std::io::Write as _;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, error::ErrorKind};
use tanren_app_services::{ActorTokenVerifier, RequestContext};
use tanren_contract::{ContractError, ErrorCode, ErrorResponse};
use tanren_observability::{ObservabilityError, init_tracing, sanitize_error_for_log};
use uuid::Uuid;

const ACTOR_TOKEN_ENV_VAR: &str = "TANREN_ACTOR_TOKEN";

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

    /// Read actor JWT from stdin (preferred for secret-safe invocation).
    #[arg(long, default_value_t = false, global = true)]
    actor_token_stdin: bool,

    /// Read actor JWT from this file path.
    #[arg(long, global = true)]
    actor_token_file: Option<PathBuf>,

    /// Path to Ed25519 public key PEM for token verification.
    #[arg(long, global = true)]
    actor_public_key_file: Option<PathBuf>,

    /// Required JWT issuer claim.
    #[arg(long, global = true)]
    token_issuer: Option<String>,

    /// Required JWT audience claim.
    #[arg(long, global = true)]
    token_audience: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Manage dispatches.
    #[command(subcommand)]
    Dispatch(commands::dispatch::DispatchCommand),
    /// Manage database schema.
    #[command(subcommand)]
    Db(DbCommand),
}

#[derive(Debug, Subcommand)]
enum DbCommand {
    /// Apply all pending schema migrations.
    Migrate,
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
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            if matches!(
                err.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) {
                err.print()?;
                return Ok(());
            }
            return Err(anyhow::Error::new(clap_error_to_response(&err)));
        }
    };

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

    let Cli {
        database_url,
        log_level: _,
        actor_token_stdin,
        actor_token_file,
        actor_public_key_file,
        token_issuer,
        token_audience,
        command,
    } = cli;

    match command {
        Commands::Db(DbCommand::Migrate) => {
            tanren_app_services::compose::run_migrations(&database_url)
                .await
                .map_err(|err| {
                    anyhow::Error::new(tanren_app_services::error::map_store_error(&err))
                })?;
            print_json(&serde_json::json!({ "status": "migrated" }))
        }
        Commands::Dispatch(cmd) => {
            let context = resolve_request_context(
                actor_token_stdin,
                actor_token_file.as_ref(),
                actor_public_key_file.as_ref(),
                token_issuer.as_deref(),
                token_audience.as_deref(),
            )?;
            let service = if cmd.requires_write_store() {
                tanren_app_services::compose::build_dispatch_service_for_write(&database_url)
                    .await
                    .map_err(|err| {
                        anyhow::Error::new(tanren_app_services::error::map_store_error(&err))
                    })?
            } else {
                tanren_app_services::compose::build_dispatch_service_for_read(&database_url)
                    .await
                    .map_err(|err| {
                        anyhow::Error::new(tanren_app_services::error::map_store_error(&err))
                    })?
            };
            commands::dispatch::handle(cmd, &service, &context).await
        }
    }
}

fn resolve_request_context(
    actor_token_stdin: bool,
    actor_token_file: Option<&PathBuf>,
    actor_public_key_file: Option<&PathBuf>,
    token_issuer: Option<&str>,
    token_audience: Option<&str>,
) -> Result<RequestContext, anyhow::Error> {
    let token = resolve_actor_token(actor_token_stdin, actor_token_file)?;
    let issuer = token_issuer.ok_or_else(|| {
        anyhow::Error::new(ErrorResponse::from(ContractError::InvalidField {
            field: "token_issuer".to_owned(),
            reason: "missing required --token-issuer".to_owned(),
        }))
    })?;
    let audience = token_audience.ok_or_else(|| {
        anyhow::Error::new(ErrorResponse::from(ContractError::InvalidField {
            field: "token_audience".to_owned(),
            reason: "missing required --token-audience".to_owned(),
        }))
    })?;
    let key_path = actor_public_key_file.ok_or_else(|| {
        anyhow::Error::new(ErrorResponse::from(ContractError::InvalidField {
            field: "actor_public_key_file".to_owned(),
            reason: "missing required --actor-public-key-file".to_owned(),
        }))
    })?;

    let public_key_pem = std::fs::read_to_string(key_path).map_err(|err| {
        anyhow::Error::new(ErrorResponse::from(ContractError::InvalidField {
            field: "actor_public_key_file".to_owned(),
            reason: format!(
                "failed to read public key file `{}`: {err}",
                key_path.display()
            ),
        }))
    })?;

    let verifier = ActorTokenVerifier::from_ed25519_pem(&public_key_pem, issuer, audience)
        .map_err(|err| anyhow::Error::new(ErrorResponse::from(err)))?;

    verifier
        .verify(token.as_str())
        .map_err(|err| anyhow::Error::new(ErrorResponse::from(err)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActorTokenSource {
    Stdin,
    File,
    Env,
}

fn resolve_actor_token(
    actor_token_stdin: bool,
    actor_token_file: Option<&PathBuf>,
) -> Result<String, anyhow::Error> {
    if actor_token_stdin && actor_token_file.is_some() {
        return Err(anyhow::Error::new(ErrorResponse::from(
            ContractError::InvalidField {
                field: "actor_token".to_owned(),
                reason:
                    "exactly one token source is allowed; use either --actor-token-stdin or --actor-token-file"
                        .to_owned(),
            },
        )));
    }

    if actor_token_stdin {
        let token = read_actor_token_from_stdin()?;
        return normalize_actor_token(&token, ActorTokenSource::Stdin);
    }

    if let Some(path) = actor_token_file {
        let token = std::fs::read_to_string(path).map_err(|err| {
            anyhow::Error::new(ErrorResponse::from(ContractError::InvalidField {
                field: "actor_token".to_owned(),
                reason: format!(
                    "failed to read actor token file `{}`: {err}",
                    path.display()
                ),
            }))
        })?;
        return normalize_actor_token(&token, ActorTokenSource::File);
    }

    if let Ok(token) = std::env::var(ACTOR_TOKEN_ENV_VAR) {
        return normalize_actor_token(&token, ActorTokenSource::Env);
    }

    Err(anyhow::Error::new(ErrorResponse::from(
        ContractError::InvalidField {
            field: "actor_token".to_owned(),
            reason: format!(
                "missing actor token source (use --actor-token-stdin, --actor-token-file, or {ACTOR_TOKEN_ENV_VAR})"
            ),
        },
    )))
}

fn read_actor_token_from_stdin() -> Result<String, anyhow::Error> {
    let mut token = String::new();
    std::io::stdin().read_to_string(&mut token).map_err(|err| {
        anyhow::Error::new(ErrorResponse::from(ContractError::InvalidField {
            field: "actor_token".to_owned(),
            reason: format!("failed to read actor token from stdin: {err}"),
        }))
    })?;
    Ok(token)
}

fn normalize_actor_token(token: &str, source: ActorTokenSource) -> Result<String, anyhow::Error> {
    let token = token.trim().to_owned();
    if token.is_empty() {
        return Err(anyhow::Error::new(ErrorResponse::from(
            ContractError::InvalidField {
                field: "actor_token".to_owned(),
                reason: "actor token source resolved to an empty token".to_owned(),
            },
        )));
    }

    tracing::info!(
        token_source = match source {
            ActorTokenSource::Stdin => "stdin",
            ActorTokenSource::File => "file",
            ActorTokenSource::Env => "env",
        },
        "resolved actor token source",
    );

    Ok(token)
}

fn into_error_response(err: anyhow::Error) -> ErrorResponse {
    match err.downcast::<ErrorResponse>() {
        Ok(er) => er,
        Err(other) => {
            let correlation_id = Uuid::now_v7();
            let sanitized = sanitize_error_for_log(&other.to_string());
            tracing::error!(
                %correlation_id,
                error = %sanitized,
                "unhandled cli failure mapped to internal error response",
            );
            ErrorResponse {
                code: ErrorCode::Internal,
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

fn print_json<T: serde::Serialize>(value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    writeln!(std::io::stdout(), "{json}")?;
    Ok(())
}
