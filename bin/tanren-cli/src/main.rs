//! Tanren CLI — composition root. Parses args, initializes tracing,
//! delegates to app-services, and dispatches to the subcommand. All
//! failures produce JSON on stderr + a non-zero exit code.
#![deny(clippy::disallowed_types, clippy::disallowed_methods)]

mod actor_token;
mod clap_error;
mod commands;
mod methodology_runtime;

use std::io::Write as _;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, error::ErrorKind};
use tanren_app_services::auth::DEFAULT_ACTOR_TOKEN_MAX_TTL_SECS;
use tanren_app_services::compose::Service;
use tanren_contract::{ContractError, ErrorCode, ErrorResponse};
use tanren_observability::{
    ObservabilityError, emit_and_build_internal_error_response, init_tracing_for_contract_io,
};

use actor_token::{resolve_actor_token, resolve_actor_token_verifier};
use clap_error::clap_error_to_response;
use commands::dispatch::{DispatchCommand, DispatchRequest};
use commands::install::{InstallArgs, run as run_install};
use commands::methodology::{MethodologyCommand, MethodologyGlobal, dispatch as run_methodology};
use methodology_runtime::load_methodology_runtime_settings;

/// Tanren — agent orchestration control plane.
#[derive(Debug, Parser)]
#[command(name = "tanren-cli", version, about)]
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
    /// Dispatch lifecycle operations.
    #[command(subcommand)]
    Dispatch(DispatchCommand),
    /// Manage database schema.
    #[command(subcommand)]
    Db(DbCommand),
    /// Render the methodology command catalog and bundled standards
    /// to their configured targets per `tanren.yml`.
    Install(InstallArgs),
    /// Methodology tool surface (CLI fallback for the MCP catalog).
    /// Each subcommand maps 1:1 to a tool in
    /// `docs/architecture/subsystems/tools.md` §3.
    #[command(flatten_help = true)]
    Methodology {
        #[command(flatten)]
        global: MethodologyGlobal,
        #[command(subcommand)]
        command: MethodologyCommand,
    },
}

#[derive(Debug, Subcommand)]
enum DbCommand {
    /// Apply all pending schema migrations.
    Migrate,
    /// Purge expired replay-ledger rows in bounded batches.
    ///
    /// Runs a single cycle of `ReplayPurgeService::run_once` against
    /// the configured database. Prints JSON stats on success. Safe
    /// to invoke from a cron.
    PurgeReplay {
        /// Max rows deleted per internal batch. Must be at least 1
        /// — a zero limit would produce a busy no-op loop in the
        /// purge runner.
        #[arg(long, default_value_t = 1_000, value_parser = clap::value_parser!(u64).range(1..))]
        batch_limit: u64,
        /// Minimum age (in seconds) an expired row must have before
        /// it is eligible for deletion.
        #[arg(long, default_value_t = 86_400)]
        retention_secs: u64,
    },
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    match run().await {
        Ok(code) => code,
        Err(RunError::TypedExit(code)) => std::process::ExitCode::from(code),
        Err(RunError::Other(err)) => {
            let error_response = into_error_response(err);
            if let Ok(json) = serde_json::to_string_pretty(&error_response) {
                let _ = writeln!(std::io::stderr(), "{json}");
            }
            std::process::ExitCode::FAILURE
        }
    }
}

/// Internal error envelope that preserves typed CLI exit codes
/// (`tanren-cli install`: 0/1/2/3/4; methodology subcommands: 0/2/4 per
/// `tools.md §5`). Other subcommands collapse into
/// `Other` and exit with the generic `ExitCode::FAILURE` path.
enum RunError {
    TypedExit(u8),
    Other(anyhow::Error),
}

impl From<anyhow::Error> for RunError {
    fn from(e: anyhow::Error) -> Self {
        Self::Other(e)
    }
}

fn parse_cli() -> std::result::Result<Cli, RunError> {
    match Cli::try_parse() {
        Ok(cli) => Ok(cli),
        Err(err) => {
            if matches!(
                err.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) {
                err.print().map_err(anyhow::Error::new)?;
                return Err(RunError::TypedExit(0));
            }
            Err(RunError::Other(anyhow::Error::new(clap_error_to_response(
                &err,
            ))))
        }
    }
}

fn init_cli_tracing(log_level: &str) -> std::result::Result<(), RunError> {
    match init_tracing_for_contract_io(log_level) {
        Ok(()) | Err(ObservabilityError::AlreadyInitialized) => Ok(()),
        Err(ObservabilityError::FilterParse(reason)) => Err(RunError::Other(anyhow::Error::new(
            ErrorResponse::from(ContractError::InvalidField {
                field: "log_level".to_owned(),
                reason,
            }),
        ))),
        Err(ObservabilityError::SinkSerialize(_) | ObservabilityError::SinkIo(_)) => {
            Err(RunError::Other(anyhow::Error::new(ErrorResponse {
                code: ErrorCode::Internal,
                message: "internal error".to_owned(),
                details: None,
            })))
        }
    }
}

fn install_exit(code: u8) -> std::result::Result<std::process::ExitCode, RunError> {
    if code == 0 {
        Ok(std::process::ExitCode::SUCCESS)
    } else {
        Err(RunError::TypedExit(code))
    }
}

async fn run_methodology_command(
    database_url: &str,
    global: MethodologyGlobal,
    command: MethodologyCommand,
) -> std::result::Result<std::process::ExitCode, RunError> {
    let runtime =
        load_methodology_runtime_settings(&global.methodology_config).map_err(RunError::Other)?;
    let standards = tanren_app_services::methodology::standards::load_runtime_standards(
        &runtime.standards_root,
    )
    .map_err(|err| {
        RunError::Other(anyhow::Error::new(ErrorResponse::from(
            ContractError::InvalidField {
                field: "standards_root".to_owned(),
                reason: format!("loading from {}: {err}", runtime.standards_root.display()),
            },
        )))
    })?;
    let phase_events = match (global.spec_id, global.spec_folder.clone()) {
        (Some(spec_id), Some(spec_folder)) => Some(
            tanren_app_services::methodology::service::PhaseEventsRuntime {
                spec_id,
                spec_folder,
                agent_session_id: global.agent_session_id.clone(),
            },
        ),
        _ => None,
    };
    let service = tanren_app_services::compose::build_methodology_service_with_config(
        database_url,
        runtime.required_guards,
        phase_events,
        standards,
        runtime.pillars,
        runtime.issue_provider,
        runtime.runtime_tuning,
    )
    .await
    .map_err(|err| {
        RunError::Other(anyhow::Error::new(
            tanren_app_services::error::map_store_error(&err),
        ))
    })?;
    install_exit(run_methodology(&service, &global, command).await)
}

async fn run() -> std::result::Result<std::process::ExitCode, RunError> {
    let cli = match parse_cli() {
        Ok(cli) => cli,
        Err(RunError::TypedExit(0)) => return Ok(std::process::ExitCode::SUCCESS),
        Err(err) => return Err(err),
    };
    init_cli_tracing(&cli.log_level)?;

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

    // Install and Methodology are factored out because they have
    // typed exit-code contracts distinct from the generic
    // `ErrorResponse` path used by every other subcommand.
    match command {
        Commands::Install(args) => return install_exit(run_install(&args)),
        Commands::Methodology { global, command } => {
            return run_methodology_command(&database_url, global, command).await;
        }
        _ => {}
    }

    let auth = AuthInputs {
        actor_token_stdin,
        actor_token_file,
        actor_public_key_file,
        token_issuer,
        token_audience,
        actor_token_max_ttl_secs,
    };
    dispatch_non_install(command, &database_url, &auth)
        .await
        .map(|()| std::process::ExitCode::SUCCESS)
        .map_err(RunError::Other)
}

/// Bundled actor-token inputs. Keeps `dispatch_non_install`'s arity
/// within workspace clippy thresholds.
struct AuthInputs {
    actor_token_stdin: bool,
    actor_token_file: Option<PathBuf>,
    actor_public_key_file: Option<PathBuf>,
    token_issuer: Option<String>,
    token_audience: Option<String>,
    actor_token_max_ttl_secs: u64,
}

async fn dispatch_non_install(
    command: Commands,
    database_url: &str,
    auth: &AuthInputs,
) -> Result<()> {
    match command {
        Commands::Db(DbCommand::Migrate) => {
            tanren_app_services::compose::run_migrations(database_url)
                .await
                .map_err(|err| {
                    anyhow::Error::new(tanren_app_services::error::map_store_error(&err))
                })?;
            print_json(&serde_json::json!({ "status": "migrated" }))
        }
        Commands::Db(DbCommand::PurgeReplay {
            batch_limit,
            retention_secs,
        }) => {
            let cfg = tanren_app_services::ReplayPurgeConfig {
                batch_limit,
                retention: std::time::Duration::from_secs(retention_secs),
                ..tanren_app_services::ReplayPurgeConfig::default()
            };
            let stats = tanren_app_services::compose::purge_replay_tokens_once(database_url, cfg)
                .await
                .map_err(|err| {
                    anyhow::Error::new(tanren_app_services::error::map_store_error(&err))
                })?;
            print_json(&stats)
        }
        Commands::Dispatch(dispatch_cmd) => {
            let (context, replay_guard) = authenticate(
                auth.actor_token_stdin,
                auth.actor_token_file.as_ref(),
                auth.actor_public_key_file.as_ref(),
                auth.token_issuer.as_deref(),
                auth.token_audience.as_deref(),
                auth.actor_token_max_ttl_secs,
            )?;
            match dispatch_cmd.split() {
                DispatchRequest::Read(cmd) => {
                    let service = build_read_service(database_url).await?;
                    commands::dispatch::handle_read(cmd, &service, &context).await
                }
                DispatchRequest::Mutation(cmd) => {
                    let service = build_write_service(database_url).await?;
                    commands::dispatch::handle_mutation(cmd, &service, &context, &replay_guard)
                        .await
                }
            }
        }
        Commands::Install(_) | Commands::Methodology { .. } => {
            unreachable!("handled in run()")
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

fn into_error_response(err: anyhow::Error) -> ErrorResponse {
    match err.downcast::<ErrorResponse>() {
        Ok(er) => er,
        Err(other) => {
            emit_and_build_internal_error_response("tanren-cli", "internal", &other.to_string())
        }
    }
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    writeln!(std::io::stdout(), "{json}")?;
    Ok(())
}
