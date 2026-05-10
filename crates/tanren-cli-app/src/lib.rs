//! Tanren scriptable command-line client — runtime library.
//!
//! R-0001 (sub-8) promotes the runtime out of `bin/tanren-cli/src/main.rs`
//! per the thin-binary-crate profile
//! (`profiles/rust-cargo/architecture/thin-binary-crate.md`). The binary
//! shrinks to a wiring shell that initializes tracing and calls [`run`];
//! everything below — `clap` parsing, account-flow dispatch, session
//! persistence — lives here so the BDD harness can depend on it directly
//! without spinning up a child process.
//!
//! The CLI continues to receive bearer-mode `SessionView` responses from
//! `tanren-app-services` (no cookie jar to use); the cookie envelope
//! lives only on the api-app surface.

use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use secrecy::SecretString;
use tanren_app_services::{AppServiceError, Handlers, Store};
use tanren_contract::{AcceptInvitationRequest, SignInRequest, SignUpRequest};
use tanren_identity_policy::{Email, InvitationToken};

mod error;
mod install;
#[cfg(feature = "test-hooks")]
pub mod test_hooks;

use crate::error::CliAppError;

const SESSION_FILE_ENV: &str = "TANREN_SESSION_FILE";

/// Top-level CLI shape. Equivalent to the historical `Cli` struct in
/// `bin/tanren-cli/src/main.rs`; renamed to `Config` so it lines up with
/// the thin-binary-crate convention (`bin/X/src/main.rs` parses a
/// `Config` and calls `tanren_X_app::run(config)`).
#[derive(Debug, Parser)]
#[command(
    name = "tanren-cli",
    version,
    about = "Tanren scriptable command-line client"
)]
pub struct Config {
    #[command(subcommand)]
    command: Option<Command>,
}

impl Config {
    /// Parse a [`Config`] from the process arguments.
    #[must_use]
    pub fn parse_from_env() -> Self {
        Self::parse()
    }
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
    /// Account flow: self-signup, sign-in, accept-invitation.
    Account {
        #[command(subcommand)]
        action: AccountAction,
    },
    /// Bootstrap Tanren assets into a repository.
    Install(install::InstallCommand),
    /// Read-only drift check for install-managed repository assets.
    Drift(install::DriftCommand),
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

#[derive(Debug, Subcommand)]
enum AccountAction {
    /// Create a personal account (or, with `--invitation`, accept an
    /// invitation and join the inviting org).
    Create {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Email to register.
        #[arg(long)]
        identifier: String,
        /// Password.
        #[arg(long)]
        password: String,
        /// Display name.
        #[arg(long, default_value_t = String::from("Tanren user"))]
        display_name: String,
        /// Optional invitation token. When supplied, the new account
        /// joins the inviting org instead of being a personal account.
        #[arg(long)]
        invitation: Option<String>,
    },
    /// Sign in to an existing account and persist the session.
    SignIn {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Email to sign in with.
        #[arg(long)]
        identifier: String,
        /// Password.
        #[arg(long)]
        password: String,
    },
}

/// Run the CLI to completion. Returns an [`ExitCode`] so the binary
/// `main` can return it directly without re-encoding error context.
#[must_use]
pub fn run(config: Config) -> ExitCode {
    let result = dispatch_command(config);
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            let stderr = std::io::stderr();
            let mut handle = stderr.lock();
            let _ = writeln!(handle, "{err}");
            ExitCode::from(1)
        }
    }
}

fn dispatch_command(config: Config) -> Result<(), CliAppError> {
    match config.command {
        None | Some(Command::Health) => {
            print_health()?;
        }
        Some(Command::Migrate {
            action: MigrateAction::Up { database_url },
        }) => {
            run_migrate_up(&database_url)?;
        }
        Some(Command::Account { action }) => {
            dispatch_account(action)?;
        }
        Some(Command::Install(command)) => {
            command.run()?;
        }
        Some(Command::Drift(command)) => {
            command.run()?;
        }
    }
    Ok(())
}

fn print_health() -> Result<(), CliAppError> {
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
    .map_err(|source| CliAppError::StdoutWriteFailure {
        target: "health report",
        source,
    })?;
    Ok(())
}

fn run_migrate_up(database_url: &str) -> Result<(), CliAppError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|source| CliAppError::RuntimeBuildFailure { source })?;
    runtime.block_on(async {
        Handlers::new()
            .migrate(database_url)
            .await
            .map_err(|source| CliAppError::MigrationFailure { source })
    })?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(handle, "migrations: applied").map_err(|source| CliAppError::StdoutWriteFailure {
        target: "migrate report",
        source,
    })?;
    Ok(())
}

fn dispatch_account(action: AccountAction) -> Result<(), CliAppError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|source| CliAppError::RuntimeBuildFailure { source })?;
    runtime.block_on(run_account(action))
}

async fn run_account(action: AccountAction) -> Result<(), CliAppError> {
    let handlers = Handlers::new();
    match action {
        AccountAction::Create {
            database_url,
            identifier,
            password,
            display_name,
            invitation,
        } => {
            let password = SecretString::from(password);
            run_account_create(
                &handlers,
                &database_url,
                &identifier,
                password,
                display_name,
                invitation,
            )
            .await?;
        }
        AccountAction::SignIn {
            database_url,
            identifier,
            password,
        } => {
            let password = SecretString::from(password);
            run_account_sign_in(&handlers, &database_url, &identifier, password).await?;
        }
    }
    Ok(())
}

async fn run_account_create(
    handlers: &Handlers,
    database_url: &str,
    identifier: &str,
    password: SecretString,
    display_name: String,
    invitation: Option<String>,
) -> Result<(), CliAppError> {
    let store = connect_store(database_url).await?;
    let email = parse_identifier_email(identifier)?;
    match invitation {
        None => {
            let response = handlers
                .sign_up(
                    &store,
                    SignUpRequest {
                        email,
                        password,
                        display_name,
                    },
                )
                .await
                .map_err(CliAppError::from_account_service_error)?;
            persist_session(response.session.token.expose_secret())?;
            write_stdout(
                "sign-up result",
                &format!(
                    "account_id={} session={}",
                    response.account.id,
                    response.session.token.expose_secret()
                ),
            )?;
        }
        Some(token) => {
            let invitation_token = parse_invitation_token(&token)?;
            let response = handlers
                .accept_invitation(
                    &store,
                    AcceptInvitationRequest {
                        invitation_token,
                        email,
                        password,
                        display_name,
                    },
                )
                .await
                .map_err(CliAppError::from_account_service_error)?;
            persist_session(response.session.token.expose_secret())?;
            write_stdout(
                "invitation-acceptance result",
                &format!(
                    "account_id={} session={} joined_org={}",
                    response.account.id,
                    response.session.token.expose_secret(),
                    response.joined_org
                ),
            )?;
        }
    }
    Ok(())
}

async fn run_account_sign_in(
    handlers: &Handlers,
    database_url: &str,
    identifier: &str,
    password: SecretString,
) -> Result<(), CliAppError> {
    let store = connect_store(database_url).await?;
    let email = parse_identifier_email(identifier)?;
    let response = handlers
        .sign_in(&store, SignInRequest { email, password })
        .await
        .map_err(CliAppError::from_account_service_error)?;
    persist_session(response.session.token.expose_secret())?;
    write_stdout(
        "sign-in result",
        &format!(
            "account_id={} session={}",
            response.account.id,
            response.session.token.expose_secret()
        ),
    )?;
    Ok(())
}

async fn connect_store(database_url: &str) -> Result<Store, CliAppError> {
    Store::connect(database_url)
        .await
        .map_err(AppServiceError::from)
        .map_err(CliAppError::from_account_service_error)
}

fn parse_identifier_email(identifier: &str) -> Result<Email, CliAppError> {
    Email::parse(identifier).map_err(|source| CliAppError::IdentifierParseFailure { source })
}

fn parse_invitation_token(raw: &str) -> Result<InvitationToken, CliAppError> {
    InvitationToken::parse(raw)
        .map_err(|source| CliAppError::InvitationTokenParseFailure { source })
}

fn write_stdout(target: &'static str, line: &str) -> Result<(), CliAppError> {
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(handle, "{line}").map_err(|source| CliAppError::StdoutWriteFailure { target, source })
}

fn session_path() -> PathBuf {
    if let Ok(explicit) = env::var(SESSION_FILE_ENV) {
        if !explicit.is_empty() {
            return PathBuf::from(explicit);
        }
    }
    let base = env::var("XDG_STATE_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map_or_else(
            || {
                env::var("HOME").ok().map_or_else(
                    || PathBuf::from("."),
                    |home| PathBuf::from(home).join(".local/state"),
                )
            },
            PathBuf::from,
        );
    base.join("tanren").join("session")
}

fn persist_session(token: &str) -> Result<(), CliAppError> {
    let path = session_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| {
            CliAppError::SessionDirectoryCreateFailure {
                path: parent.display().to_string(),
                source,
            }
        })?;
    }
    fs::write(&path, token).map_err(|source| CliAppError::SessionWriteFailure {
        path: path.display().to_string(),
        source,
    })?;
    Ok(())
}
