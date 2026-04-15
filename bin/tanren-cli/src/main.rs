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
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, error::ErrorKind};
use tanren_app_services::{ActorTokenVerifier, RequestContext};
use tanren_contract::{ContractError, ErrorCode, ErrorResponse};
use tanren_observability::{
    ObservabilityError, emit_correlated_internal_error, init_tracing_for_contract_io,
};
use uuid::Uuid;

const ACTOR_TOKEN_ENV_VAR: &str = "TANREN_ACTOR_TOKEN";
type EmitCorrelatedInternalError = fn(&str, &str, Uuid, &str) -> Result<(), ObservabilityError>;

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

    match init_tracing_for_contract_io(&cli.log_level) {
        Ok(()) | Err(ObservabilityError::AlreadyInitialized) => {}
        Err(ObservabilityError::FilterParse(reason)) => {
            return Err(anyhow::Error::new(ErrorResponse::from(
                ContractError::InvalidField {
                    field: "log_level".to_owned(),
                    reason,
                },
            )));
        }
        Err(ObservabilityError::SinkSerialize(_) | ObservabilityError::SinkIo(_)) => {
            return Err(anyhow::Error::new(ErrorResponse {
                code: ErrorCode::Internal,
                message: "internal error".to_owned(),
                details: None,
            }));
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
        let raw_error = format!(
            "failed to read actor public key file `{}`: {err}",
            key_path.display()
        );
        let _ = emit_correlated_internal_error(
            "tanren_cli",
            "invalid_actor_public_key",
            Uuid::now_v7(),
            &raw_error,
        );
        anyhow::Error::new(ErrorResponse::from(ContractError::InvalidField {
            field: "actor_public_key_file".to_owned(),
            reason: "invalid actor public key".to_owned(),
        }))
    })?;

    let verifier = ActorTokenVerifier::from_ed25519_pem(&public_key_pem, issuer, audience)
        .map_err(|err| anyhow::Error::new(ErrorResponse::from(err)))?;

    verifier
        .verify(token.as_str())
        .map_err(|err| anyhow::Error::new(ErrorResponse::from(err)))
}

fn resolve_actor_token(
    actor_token_stdin: bool,
    actor_token_file: Option<&PathBuf>,
) -> Result<String, anyhow::Error> {
    let env_token = std::env::var(ACTOR_TOKEN_ENV_VAR)
        .ok()
        .filter(|token| !token.trim().is_empty());
    let selected_source_count = u8::from(actor_token_stdin)
        + u8::from(actor_token_file.is_some())
        + u8::from(env_token.is_some());

    if selected_source_count > 1 {
        return Err(anyhow::Error::new(ErrorResponse::from(
            ContractError::InvalidField {
                field: "actor_token".to_owned(),
                reason: format!(
                    "exactly one token source is allowed; choose one of --actor-token-stdin, --actor-token-file, or {ACTOR_TOKEN_ENV_VAR}"
                ),
            },
        )));
    }

    if actor_token_stdin {
        let token = read_actor_token_from_stdin()?;
        return normalize_actor_token(&token);
    }

    if let Some(path) = actor_token_file {
        let token = read_actor_token_from_file(path)?;
        return normalize_actor_token(&token);
    }

    if let Some(token) = env_token {
        return normalize_actor_token(&token);
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

fn read_actor_token_from_file(path: &PathBuf) -> Result<String, anyhow::Error> {
    std::fs::read_to_string(path).map_err(|err| {
        actor_token_source_error(
            "invalid_actor_token_source",
            &format!(
                "failed to read actor token file `{}`: {err}",
                path.display()
            ),
        )
    })
}

fn read_actor_token_from_stdin() -> Result<String, anyhow::Error> {
    let mut stdin = std::io::stdin();
    read_actor_token_from_reader(&mut stdin)
}

fn read_actor_token_from_reader(reader: &mut dyn std::io::Read) -> Result<String, anyhow::Error> {
    let mut token = String::new();
    reader.read_to_string(&mut token).map_err(|err| {
        actor_token_source_error(
            "invalid_actor_token_source",
            &format!("failed to read actor token from stdin: {err}"),
        )
    })?;
    Ok(token)
}

fn actor_token_source_error(error_code: &str, raw_error: &str) -> anyhow::Error {
    let _ = emit_correlated_internal_error("tanren_cli", error_code, Uuid::now_v7(), raw_error);
    anyhow::Error::new(ErrorResponse::from(ContractError::InvalidField {
        field: "actor_token".to_owned(),
        reason: "invalid actor token source".to_owned(),
    }))
}

fn normalize_actor_token(token: &str) -> Result<String, anyhow::Error> {
    let token = token.trim().to_owned();
    if token.is_empty() {
        return Err(anyhow::Error::new(ErrorResponse::from(
            ContractError::InvalidField {
                field: "actor_token".to_owned(),
                reason: "actor token source resolved to an empty token".to_owned(),
            },
        )));
    }

    Ok(token)
}

fn into_error_response(err: anyhow::Error) -> ErrorResponse {
    match err.downcast::<ErrorResponse>() {
        Ok(er) => er,
        Err(other) => correlated_internal_error_response_with_emitter(
            "tanren_cli",
            "internal",
            &other.to_string(),
            emit_correlated_internal_error,
        ),
    }
}

fn correlated_internal_error_response_with_emitter(
    component: &str,
    error_code: &str,
    raw_error: &str,
    emitter: EmitCorrelatedInternalError,
) -> ErrorResponse {
    let correlation_id = Uuid::now_v7();
    let details = if emitter(component, error_code, correlation_id, raw_error).is_ok() {
        Some(serde_json::json!({
            "correlation_id": correlation_id.to_string(),
        }))
    } else {
        None
    };

    ErrorResponse {
        code: ErrorCode::Internal,
        message: "internal error".to_owned(),
        details,
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

#[cfg(test)]
mod tests {
    use std::io::{Error as IoError, ErrorKind as IoErrorKind, Read};

    use tanren_contract::ErrorCode;

    use super::{into_error_response, read_actor_token_from_reader};

    struct FailingReader;

    impl Read for FailingReader {
        fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
            Err(IoError::new(
                IoErrorKind::PermissionDenied,
                "redacted-test-io-detail",
            ))
        }
    }

    #[test]
    fn actor_token_stdin_failure_is_generic_and_redacted() {
        let mut reader = FailingReader;
        let err = read_actor_token_from_reader(&mut reader).expect_err("read should fail");
        let response = into_error_response(err);
        assert_eq!(response.code, ErrorCode::InvalidInput);
        assert!(response.message.contains("invalid actor token source"));
        assert!(!response.message.contains("stdin"));
        assert!(!response.message.contains("PermissionDenied"));
        assert!(!response.message.contains("redacted-test-io-detail"));
    }
}
