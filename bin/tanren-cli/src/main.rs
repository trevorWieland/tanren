//! Tanren CLI — the composition root for command-line operation.
//!
//! Parses args, initializes tracing, and delegates to the app-services
//! composition root for wiring, then dispatches to the appropriate
//! subcommand handler.
//!
//! All failures — including startup errors — produce deterministic JSON
//! on stderr and a non-zero exit code.
#![deny(clippy::disallowed_types, clippy::disallowed_methods)]

mod actor_token;
mod clap_error;
mod commands;

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
    /// Dispatch lifecycle operations.
    #[command(subcommand)]
    Dispatch(DispatchCommand),
    /// Manage database schema.
    #[command(subcommand)]
    Db(DbCommand),
    /// Render the methodology command catalog and bundled standards
    /// to their configured targets per `tanren.yml`.
    Install(InstallArgs),
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
        Commands::Db(DbCommand::PurgeReplay {
            batch_limit,
            retention_secs,
        }) => {
            let cfg = tanren_app_services::ReplayPurgeConfig {
                batch_limit,
                retention: std::time::Duration::from_secs(retention_secs),
                ..tanren_app_services::ReplayPurgeConfig::default()
            };
            let stats = tanren_app_services::compose::purge_replay_tokens_once(&database_url, cfg)
                .await
                .map_err(|err| {
                    anyhow::Error::new(tanren_app_services::error::map_store_error(&err))
                })?;
            print_json(&stats)
        }
        Commands::Dispatch(dispatch_cmd) => {
            let (context, replay_guard) = authenticate(
                actor_token_stdin,
                actor_token_file.as_ref(),
                actor_public_key_file.as_ref(),
                token_issuer.as_deref(),
                token_audience.as_deref(),
                actor_token_max_ttl_secs,
            )?;
            match dispatch_cmd.split() {
                DispatchRequest::Read(cmd) => {
                    let service = build_read_service(&database_url).await?;
                    commands::dispatch::handle_read(cmd, &service, &context).await
                }
                DispatchRequest::Mutation(cmd) => {
                    let service = build_write_service(&database_url).await?;
                    commands::dispatch::handle_mutation(cmd, &service, &context, &replay_guard)
                        .await
                }
            }
        }
        Commands::Install(args) => {
            let code = run_install(&args);
            if code == 0 {
                Ok(())
            } else {
                Err(anyhow::anyhow!("install exited with code {code}"))
            }
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
            emit_and_build_internal_error_response("tanren_cli", "internal", &other.to_string())
        }
    }
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    writeln!(std::io::stdout(), "{json}")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::{Error as IoError, ErrorKind as IoErrorKind, Read};

    use clap::{CommandFactory, Parser};
    use tanren_app_services::auth::DEFAULT_ACTOR_TOKEN_MAX_BYTES;
    use tanren_contract::{CliParseReasonCode, ErrorCode, ErrorDetails};

    use super::actor_token::read_actor_token_from_reader;
    use super::clap_error::{ALLOWED_ARG_FIELDS, clap_error_to_response};
    use super::{Cli, into_error_response};

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
        let oversized = "x".repeat(DEFAULT_ACTOR_TOKEN_MAX_BYTES + 1);
        let mut reader = std::io::Cursor::new(oversized);
        let err = read_actor_token_from_reader(&mut reader).expect_err("oversized input");
        let response = into_error_response(err);
        assert_eq!(response.code, ErrorCode::InvalidInput);
        assert!(response.message.contains("invalid actor token source"));
    }

    #[test]
    fn allowed_arg_fields_covers_every_declared_long_flag() {
        use std::collections::BTreeSet;

        fn collect_longs(cmd: &clap::Command, acc: &mut BTreeSet<String>) {
            for arg in cmd.get_arguments() {
                if let Some(long) = arg.get_long() {
                    acc.insert(long.replace('-', "_"));
                }
            }
            for sub in cmd.get_subcommands() {
                collect_longs(sub, acc);
            }
        }

        let mut declared = BTreeSet::new();
        collect_longs(&Cli::command(), &mut declared);

        let allowlist: BTreeSet<String> =
            ALLOWED_ARG_FIELDS.iter().map(|s| (*s).to_owned()).collect();

        for long in &declared {
            assert!(
                allowlist.contains(long),
                "declared flag --{long} is not listed in ALLOWED_ARG_FIELDS"
            );
        }
    }

    #[test]
    fn missing_required_argument_maps_to_safe_wire_response() {
        let err = Cli::try_parse_from(["tanren", "dispatch", "create"])
            .expect_err("missing --project must fail");
        let response = clap_error_to_response(&err);
        assert_eq!(response.code, ErrorCode::InvalidInput);
        assert_eq!(response.message, "invalid cli args");
        assert!(
            matches!(
                &response.details,
                Some(ErrorDetails::InvalidArgs { reason_code, .. })
                    if *reason_code == CliParseReasonCode::MissingRequiredArgument
            ),
            "expected missing_required_argument details, got {:?}",
            response.details
        );
    }

    #[test]
    fn invalid_value_does_not_echo_user_input_on_wire() {
        // User supplies a made-up phase value that also contains a
        // pretend-secret-shaped token. The wire payload must not
        // include any of that text.
        let err = Cli::try_parse_from([
            "tanren",
            "dispatch",
            "create",
            "--project",
            "p",
            "--phase",
            "sk-super-secret-value",
            "--cli",
            "claude",
            "--branch",
            "b",
            "--spec-folder",
            "s",
            "--workflow-id",
            "w",
        ])
        .expect_err("invalid --phase value must fail");
        let response = clap_error_to_response(&err);
        let json = serde_json::to_string(&response).expect("serialize");
        assert!(
            !json.contains("sk-super-secret-value"),
            "raw user value leaked into wire: {json}"
        );
        assert!(
            !json.contains("super-secret"),
            "raw user value leaked into wire: {json}"
        );
        assert_eq!(response.code, ErrorCode::InvalidInput);
        assert_eq!(response.message, "invalid cli args");
    }

    #[test]
    fn unknown_argument_with_secret_value_is_not_echoed() {
        let err = Cli::try_parse_from(["tanren", "dispatch", "list", "--secret-value=sk-1234"])
            .expect_err("unknown --secret-value must fail");
        let response = clap_error_to_response(&err);
        let json = serde_json::to_string(&response).expect("serialize");
        assert!(!json.contains("sk-1234"), "raw secret leaked: {json}");
        assert!(
            !json.contains("secret-value"),
            "unknown flag name not allowlisted must not reach wire: {json}"
        );
        assert_eq!(response.code, ErrorCode::InvalidInput);
        assert_eq!(response.message, "invalid cli args");
    }
}
