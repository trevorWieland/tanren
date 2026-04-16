//! Tanren CLI — the composition root for command-line operation.
//!
//! Parses args, initializes tracing, and delegates to the app-services
//! composition root for wiring, then dispatches to the appropriate
//! subcommand handler.
//!
//! All failures — including startup errors — produce deterministic JSON
//! on stderr and a non-zero exit code.
#![deny(clippy::disallowed_types, clippy::disallowed_methods)]

mod commands;

use std::io::{Read as _, Write as _};
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, error::ErrorKind};
use tanren_app_services::ActorTokenVerifier;
use tanren_app_services::auth::DEFAULT_ACTOR_TOKEN_MAX_TTL_SECS;
use tanren_app_services::compose::Service;
use tanren_contract::{ContractError, ErrorCode, ErrorResponse};
use tanren_observability::{
    ObservabilityError, emit_and_build_internal_error_response, emit_correlated_internal_error,
    init_tracing_for_contract_io,
};
use uuid::Uuid;

const ACTOR_TOKEN_ENV_VAR: &str = "TANREN_ACTOR_TOKEN";
const ACTOR_TOKEN_MAX_BYTES: usize = 16 * 1024;

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

    /// Path to Ed25519 public key PEM for actor token verification.
    #[arg(long, global = true)]
    actor_public_key_file: Option<PathBuf>,

    /// Required JWT issuer claim.
    #[arg(long, global = true)]
    token_issuer: Option<String>,

    /// Required JWT audience claim.
    #[arg(long, global = true)]
    token_audience: Option<String>,

    /// Maximum allowed actor token lifetime (`exp - iat`) in seconds.
    #[arg(
        long,
        global = true,
        default_value_t = DEFAULT_ACTOR_TOKEN_MAX_TTL_SECS
    )]
    actor_token_max_ttl_secs: u64,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Query dispatches (read-only, no replay guard consumed).
    #[command(subcommand, name = "dispatch-read")]
    DispatchRead(commands::dispatch::DispatchReadCommand),
    /// Mutate dispatches (consumes replay guard atomically).
    #[command(subcommand, name = "dispatch-mutation")]
    DispatchMutation(commands::dispatch::DispatchMutationCommand),
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
        actor_token_max_ttl_secs,
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
        Commands::DispatchRead(cmd) => {
            let (context, _replay_guard) = authenticate(
                actor_token_stdin,
                actor_token_file.as_ref(),
                actor_public_key_file.as_ref(),
                token_issuer.as_deref(),
                token_audience.as_deref(),
                actor_token_max_ttl_secs,
            )?;
            let service = build_read_service(&database_url).await?;
            commands::dispatch::handle_read(cmd, &service, &context).await
        }
        Commands::DispatchMutation(cmd) => {
            let (context, replay_guard) = authenticate(
                actor_token_stdin,
                actor_token_file.as_ref(),
                actor_public_key_file.as_ref(),
                token_issuer.as_deref(),
                token_audience.as_deref(),
                actor_token_max_ttl_secs,
            )?;
            let service = build_write_service(&database_url).await?;
            commands::dispatch::handle_mutation(cmd, &service, &context, &replay_guard).await
        }
    }
}

fn authenticate(
    actor_token_stdin: bool,
    actor_token_file: Option<&PathBuf>,
    actor_public_key_file: Option<&PathBuf>,
    token_issuer: Option<&str>,
    token_audience: Option<&str>,
    actor_token_max_ttl_secs: u64,
) -> Result<(
    tanren_app_services::RequestContext,
    tanren_app_services::ReplayGuard,
)> {
    let token = resolve_actor_token(actor_token_stdin, actor_token_file)?;
    let verifier = resolve_actor_token_verifier(
        actor_public_key_file,
        token_issuer,
        token_audience,
        actor_token_max_ttl_secs,
    )?;
    let token_ctx = verifier
        .verify(token.as_str())
        .map_err(|err| anyhow::Error::new(ErrorResponse::from(ContractError::from(err))))?;
    Ok(token_ctx.into_parts())
}

async fn build_read_service(database_url: &str) -> Result<Service> {
    tanren_app_services::compose::build_dispatch_service_for_read(database_url)
        .await
        .map_err(|err| anyhow::Error::new(tanren_app_services::error::map_store_error(&err)))
}

async fn build_write_service(database_url: &str) -> Result<Service> {
    tanren_app_services::compose::build_dispatch_service_for_write(database_url)
        .await
        .map_err(|err| anyhow::Error::new(tanren_app_services::error::map_store_error(&err)))
}

fn resolve_actor_token_verifier(
    actor_public_key_file: Option<&PathBuf>,
    token_issuer: Option<&str>,
    token_audience: Option<&str>,
    actor_token_max_ttl_secs: u64,
) -> Result<ActorTokenVerifier, anyhow::Error> {
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
    if actor_token_max_ttl_secs == 0 {
        return Err(anyhow::Error::new(ErrorResponse::from(
            ContractError::InvalidField {
                field: "actor_token_max_ttl_secs".to_owned(),
                reason: "must be >= 1".to_owned(),
            },
        )));
    }

    if let Some(path) = actor_public_key_file {
        return ActorTokenVerifier::from_public_key_file(
            path,
            issuer,
            audience,
            actor_token_max_ttl_secs,
        )
        .map_err(|err| anyhow::Error::new(ErrorResponse::from(err)));
    }

    Err(anyhow::Error::new(ErrorResponse::from(
        ContractError::InvalidField {
            field: "actor_public_key".to_owned(),
            reason: "missing required --actor-public-key-file".to_owned(),
        },
    )))
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
    let mut file = std::fs::File::open(path).map_err(|err| {
        actor_token_source_error(
            "invalid_actor_token_source",
            &format!(
                "failed to read actor token file `{}`: {err}",
                path.display()
            ),
        )
    })?;
    read_actor_token_from_reader_with_source(&mut file, &format!("file `{}`", path.display()))
}

fn read_actor_token_from_stdin() -> Result<String, anyhow::Error> {
    let mut stdin = std::io::stdin();
    read_actor_token_from_reader_with_source(&mut stdin, "stdin")
}

#[cfg(test)]
fn read_actor_token_from_reader(reader: &mut dyn std::io::Read) -> Result<String, anyhow::Error> {
    read_actor_token_from_reader_with_source(reader, "stdin")
}

fn read_actor_token_from_reader_with_source(
    reader: &mut dyn std::io::Read,
    source: &str,
) -> Result<String, anyhow::Error> {
    let mut limited = reader.take((ACTOR_TOKEN_MAX_BYTES as u64).saturating_add(1));
    let mut token_bytes = Vec::new();
    limited.read_to_end(&mut token_bytes).map_err(|err| {
        actor_token_source_error(
            "invalid_actor_token_source",
            &format!("failed to read actor token from {source}: {err}"),
        )
    })?;
    if token_bytes.len() > ACTOR_TOKEN_MAX_BYTES {
        return Err(actor_token_source_error(
            "invalid_actor_token_source",
            &format!("actor token from {source} exceeds {ACTOR_TOKEN_MAX_BYTES} bytes"),
        ));
    }
    String::from_utf8(token_bytes).map_err(|err| {
        actor_token_source_error(
            "invalid_actor_token_source",
            &format!("actor token from {source} is not valid utf-8: {err}"),
        )
    })
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
    if token.len() > ACTOR_TOKEN_MAX_BYTES {
        return Err(actor_token_source_error(
            "invalid_actor_token_source",
            &format!("actor token exceeds {ACTOR_TOKEN_MAX_BYTES} bytes after normalization"),
        ));
    }

    Ok(token)
}

fn into_error_response(err: anyhow::Error) -> ErrorResponse {
    match err.downcast::<ErrorResponse>() {
        Ok(er) => er,
        Err(other) => {
            emit_and_build_internal_error_response("tanren_cli", "internal", &other.to_string())
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

#[cfg(test)]
mod tests {
    use std::io::{Error as IoError, ErrorKind as IoErrorKind, Read};

    use tanren_contract::ErrorCode;

    use super::{ACTOR_TOKEN_MAX_BYTES, into_error_response, read_actor_token_from_reader};

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

    #[test]
    fn actor_token_stdin_overflow_is_generic_and_invalid_input() {
        let oversized = "x".repeat(ACTOR_TOKEN_MAX_BYTES + 1);
        let mut reader = std::io::Cursor::new(oversized);
        let err = read_actor_token_from_reader(&mut reader).expect_err("oversized input");
        let response = into_error_response(err);
        assert_eq!(response.code, ErrorCode::InvalidInput);
        assert!(response.message.contains("invalid actor token source"));
    }
}
