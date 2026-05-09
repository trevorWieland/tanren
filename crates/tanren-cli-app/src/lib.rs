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

mod session;

use std::env;
use std::io::Write;
use std::process::ExitCode;
use std::str::FromStr;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use secrecy::SecretString;
use tanren_app_services::{AppServiceError, Clock, Handlers, Store};
use tanren_contract::{
    AcceptInvitationRequest, ListActiveAccountsRequest, SignInRequest, SignUpRequest,
    SwitchActiveAccountRequest,
};
use tanren_identity_policy::{AccountId, Email, InvitationToken};
use uuid::Uuid;

use crate::session::{active_context_from_session, persist_session, set_active_account};

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
    /// List signed-in accounts and show which one is currently active
    /// for this CLI window context.
    ListActive {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
    },
    /// Switch the active account for this CLI window context.
    SwitchActive {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Account id that should become active.
        #[arg(long)]
        target_account_id: String,
    },
}

/// Run the CLI to completion. Returns an [`ExitCode`] so the binary
/// `main` can return it directly without re-encoding error context.
#[must_use]
pub fn run(config: Config) -> ExitCode {
    let result = match config.command {
        None | Some(Command::Health) => print_health(),
        Some(Command::Migrate {
            action: MigrateAction::Up { database_url },
        }) => run_migrate_up(&database_url),
        Some(Command::Account { action }) => dispatch_account(action),
    };
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

fn dispatch_account(action: AccountAction) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;
    runtime.block_on(run_account(action))
}

async fn run_account(action: AccountAction) -> Result<()> {
    let clock = Clock::default();
    let handlers = Handlers::with_clock(clock.clone());
    match action {
        AccountAction::Create {
            database_url,
            identifier,
            password,
            display_name,
            invitation,
        } => {
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
        } => run_account_sign_in(&handlers, &database_url, &identifier, password).await?,
        AccountAction::ListActive { database_url } => {
            run_account_list_active(&handlers, &clock, &database_url).await?;
        }
        AccountAction::SwitchActive {
            database_url,
            target_account_id,
        } => {
            run_account_switch_active(&handlers, &clock, &database_url, &target_account_id).await?;
        }
    }
    Ok(())
}

async fn run_account_create(
    handlers: &Handlers,
    database_url: &str,
    identifier: &str,
    password: String,
    display_name: String,
    invitation: Option<String>,
) -> Result<()> {
    let store = Store::connect(database_url)
        .await
        .context("connect to store")?;
    let email = Email::parse(identifier).context("parse --identifier as email")?;
    let password = SecretString::from(password);
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
                .map_err(account_error)?;
            persist_session(response.account.id, response.session.token.expose_secret())?;
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            writeln!(
                handle,
                "account_id={id} session={token}",
                id = response.account.id,
                token = response.session.token.expose_secret(),
            )
            .context("write sign-up result")?;
        }
        Some(token) => {
            let invitation_token =
                InvitationToken::parse(&token).context("parse --invitation as invitation token")?;
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
                .map_err(account_error)?;
            persist_session(response.account.id, response.session.token.expose_secret())?;
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            writeln!(
                handle,
                "account_id={id} session={token} joined_org={org}",
                id = response.account.id,
                token = response.session.token.expose_secret(),
                org = response.joined_org,
            )
            .context("write invitation-acceptance result")?;
        }
    }
    Ok(())
}

async fn run_account_sign_in(
    handlers: &Handlers,
    database_url: &str,
    identifier: &str,
    password: String,
) -> Result<()> {
    let store = Store::connect(database_url)
        .await
        .context("connect to store")?;
    let email = Email::parse(identifier).context("parse --identifier as email")?;
    let password = SecretString::from(password);
    let response = handlers
        .sign_in(&store, SignInRequest { email, password })
        .await
        .map_err(account_error)?;
    persist_session(response.account.id, response.session.token.expose_secret())?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(
        handle,
        "account_id={id} session={token}",
        id = response.account.id,
        token = response.session.token.expose_secret(),
    )
    .context("write sign-in result")?;
    Ok(())
}

async fn run_account_list_active(
    handlers: &Handlers,
    clock: &Clock,
    database_url: &str,
) -> Result<()> {
    let store = Store::connect(database_url)
        .await
        .context("connect to store")?;
    let context = active_context_from_session(&store, clock).await?;
    let response = handlers
        .list_active_accounts(&store, &context, ListActiveAccountsRequest::default())
        .await
        .map_err(account_error)?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    let body = serde_json::to_string(&response).context("serialize active-account list result")?;
    writeln!(handle, "{body}").context("write active-account list result")?;
    Ok(())
}

async fn run_account_switch_active(
    handlers: &Handlers,
    clock: &Clock,
    database_url: &str,
    target_account_id: &str,
) -> Result<()> {
    let store = Store::connect(database_url)
        .await
        .context("connect to store")?;
    let context = active_context_from_session(&store, clock).await?;
    let target_account_id = AccountId::from(
        Uuid::from_str(target_account_id).context("parse --target-account-id as uuid")?,
    );
    let response = handlers
        .switch_active_account(
            &store,
            &context,
            SwitchActiveAccountRequest { target_account_id },
        )
        .await
        .map_err(account_error)?;
    set_active_account(response.active_account_id)?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    let body =
        serde_json::to_string(&response).context("serialize active-account switch result")?;
    writeln!(handle, "{body}").context("write active-account switch result")?;
    Ok(())
}

fn account_error(err: AppServiceError) -> anyhow::Error {
    match err {
        AppServiceError::Account(reason) => {
            anyhow::anyhow!("error: {} — {}", reason.code(), reason.summary())
        }
        AppServiceError::InvalidInput(message) => {
            anyhow::anyhow!("error: validation_failed — {message}")
        }
        AppServiceError::Store(err) => {
            anyhow::anyhow!("error: internal_error — {err}")
        }
        _ => anyhow::anyhow!("error: internal_error — unknown app-service failure"),
    }
}
