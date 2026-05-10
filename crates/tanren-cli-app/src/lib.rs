//! Tanren scriptable command-line client — runtime library.
//!
//! R-0001 (sub-8) promotes the runtime out of `bin/tanren-cli/src/main.rs`
//! per the thin-binary-crate profile. The binary shrinks to a wiring
//! shell that initializes tracing and calls [`run`];
//! everything below — `clap` parsing, account-flow dispatch, session
//! persistence — lives here so the BDD harness can depend on it directly
//! without spinning up a child process.

mod permissions_output;

use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};
use secrecy::SecretString;
use tanren_app_services::{AccountStore, AppServiceError, Handlers, MyPermissionsContext, Store};
use tanren_contract::{
    AcceptInvitationRequest, MY_PERMISSIONS_DEFAULT_LIMIT, MyPermissionsRequest, SignInRequest,
    SignUpRequest,
};
use tanren_identity_policy::{AccountId, Email, InvitationToken, SessionToken};
use uuid::Uuid;

use crate::permissions_output::print_permissions;

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
    /// Account flow: self-signup, sign-in, accept-invitation, and
    /// self-permission introspection.
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
    /// Read the signed-in account's effective permissions.
    MyPermissions {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Optional explicit target account id. Supplying a different id
        /// than the signed-in account is rejected as `permission_denied`.
        #[arg(long)]
        target_account_id: Option<String>,
    },
}

/// Run the CLI to completion.
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
    let handlers = Handlers::new();
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
        AccountAction::MyPermissions {
            database_url,
            target_account_id,
        } => run_account_my_permissions(&handlers, &database_url, target_account_id).await?,
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
            persist_session(response.session.token.expose_secret())?;
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
            persist_session(response.session.token.expose_secret())?;
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
    persist_session(response.session.token.expose_secret())?;
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

async fn run_account_my_permissions(
    handlers: &Handlers,
    database_url: &str,
    target_account_id: Option<String>,
) -> Result<()> {
    let store = Store::connect(database_url)
        .await
        .context("connect to store")?;
    let session_token = read_session_token()?;
    let session = AccountStore::find_session_by_token(&store, &session_token)
        .await
        .context("load persisted session from store")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "error: auth_required — session is invalid or revoked; sign in again to refresh {}",
                session_path().display()
            )
        })?;
    if session.expires_at <= Utc::now() {
        anyhow::bail!(
            "error: auth_required — session has expired; sign in again to refresh {}",
            session_path().display()
        );
    }
    let session_account_id = session.account_id;
    let requested_account_id = target_account_id
        .map(|raw| parse_account_id(&raw, "--target-account-id"))
        .transpose()?
        .unwrap_or(session_account_id);
    let response = handlers
        .my_permissions(
            &store,
            MyPermissionsContext::with_requested_account(session_account_id, requested_account_id),
            MyPermissionsRequest {
                limit: Some(MY_PERMISSIONS_DEFAULT_LIMIT),
                cursor: None,
            },
        )
        .await
        .map_err(account_error)?;
    print_permissions(&response)?;
    Ok(())
}

fn account_error(err: AppServiceError) -> anyhow::Error {
    let interface_error = err.into_interface_error();
    anyhow::anyhow!(
        "error: {} — {}",
        interface_error.code.as_str(),
        interface_error.summary
    )
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

fn persist_session(token: &str) -> Result<()> {
    let path = session_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create session dir {}", parent.display()))?;
    }
    let content = format!("version=1\ntoken={token}\n");
    fs::write(&path, content).with_context(|| format!("write session to {}", path.display()))?;
    Ok(())
}

fn read_session_token() -> Result<SessionToken> {
    let path = session_path();
    let content = fs::read_to_string(&path)
        .with_context(|| format!("read session from {}", path.display()))?;
    for line in content.lines() {
        if let Some(raw) = line.strip_prefix("token=") {
            let token = raw.trim();
            if token.is_empty() {
                anyhow::bail!(
                    "error: auth_required — session is missing token; sign in again to refresh {}",
                    path.display()
                );
            }
            return Ok(SessionToken::from_secret(SecretString::from(
                token.to_owned(),
            )));
        }
    }
    anyhow::bail!(
        "error: auth_required — session is missing token; sign in again to refresh {}",
        path.display()
    )
}

fn parse_account_id(raw: &str, context_label: &str) -> Result<AccountId> {
    let parsed = Uuid::parse_str(raw)
        .with_context(|| format!("parse {context_label} as uuid-form account id"))?;
    Ok(AccountId::new(parsed))
}
