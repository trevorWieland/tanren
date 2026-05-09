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

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use secrecy::SecretString;
use tanren_app_services::deployment_posture::{
    SetDeploymentPostureError, missing_or_expired_session_failure,
};
use tanren_app_services::{AppServiceError, Handlers, Store};
use tanren_contract::{
    AcceptInvitationRequest, DeploymentPosture, DeploymentPostureFailureReason,
    DeploymentPostureScope, SetDeploymentPostureRequest, SignInRequest, SignUpRequest,
};
use tanren_identity_policy::{
    AccountId, Email, InstallationId, InvitationToken, ProjectId, SessionToken,
};
use uuid::Uuid;

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
    /// Deployment posture flow: list supported values, read current selection, update selection.
    Posture {
        #[command(subcommand)]
        action: PostureAction,
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
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ScopeKindArg {
    Account,
    Project,
    Installation,
}

#[derive(Debug, Subcommand)]
enum PostureAction {
    /// List supported deployment postures with capability summaries.
    List,
    /// Read current deployment posture for a scope.
    Get {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Scope kind.
        #[arg(long)]
        scope_kind: ScopeKindArg,
        /// Scope identifier UUID.
        #[arg(long)]
        scope_id: String,
    },
    /// Set deployment posture for a scope.
    Set {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Scope kind.
        #[arg(long)]
        scope_kind: ScopeKindArg,
        /// Scope identifier UUID.
        #[arg(long)]
        scope_id: String,
        /// Posture value (`hosted`, `self_hosted`, `local_only`).
        #[arg(long)]
        posture: String,
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
        Some(Command::Posture { action }) => dispatch_posture(action),
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

fn dispatch_posture(action: PostureAction) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;
    runtime.block_on(run_posture(action))
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
            let store = Store::connect(&database_url)
                .await
                .context("connect to store")?;
            let email = Email::parse(&identifier).context("parse --identifier as email")?;
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
                    let payload =
                        serde_json::to_value(response).context("encode sign-up response")?;
                    write_json_line(&payload)?;
                }
                Some(token) => {
                    let invitation_token = InvitationToken::parse(&token)
                        .context("parse --invitation as invitation token")?;
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
                    let payload = serde_json::to_value(response)
                        .context("encode invitation-acceptance response")?;
                    write_json_line(&payload)?;
                }
            }
        }
        AccountAction::SignIn {
            database_url,
            identifier,
            password,
        } => {
            let store = Store::connect(&database_url)
                .await
                .context("connect to store")?;
            let email = Email::parse(&identifier).context("parse --identifier as email")?;
            let password = SecretString::from(password);
            let response = handlers
                .sign_in(&store, SignInRequest { email, password })
                .await
                .map_err(account_error)?;
            persist_session(response.session.token.expose_secret())?;
            let payload = serde_json::to_value(response).context("encode sign-in response")?;
            write_json_line(&payload)?;
        }
    }
    Ok(())
}

async fn run_posture(action: PostureAction) -> Result<()> {
    let handlers = Handlers::new();
    match action {
        PostureAction::List => {
            let payload = serde_json::to_value(handlers.list_supported_deployment_postures())
                .context("encode supported posture response")?;
            write_json_line(&payload)?;
        }
        PostureAction::Get {
            database_url,
            scope_kind,
            scope_id,
        } => {
            let store = Store::connect(&database_url)
                .await
                .context("connect to store")?;
            let scope = parse_scope(scope_kind, &scope_id)?;
            let current = handlers
                .deployment_posture(&store, scope)
                .await
                .context("read deployment posture")?;
            let payload =
                serde_json::to_value(current).context("encode current posture response")?;
            write_json_line(&payload)?;
        }
        PostureAction::Set {
            database_url,
            scope_kind,
            scope_id,
            posture,
        } => {
            let store = Store::connect(&database_url)
                .await
                .context("connect to store")?;
            let scope = parse_scope(scope_kind, &scope_id)?;
            let actor = resolve_actor_from_session(&handlers, &store).await?;
            let posture = parse_posture_value(&posture)?;
            let response = handlers
                .set_deployment_posture(
                    &store,
                    actor,
                    SetDeploymentPostureRequest { scope, posture },
                )
                .await
                .map_err(posture_error)?;
            let payload = serde_json::json!({ "current": response });
            write_json_line(&payload)?;
        }
    }
    Ok(())
}

async fn resolve_actor_from_session(handlers: &Handlers, store: &Store) -> Result<AccountId> {
    let token = load_persisted_session_token()?;
    handlers
        .resolve_active_session_account(store, &token)
        .await
        .map_err(posture_error)
}

fn parse_scope(kind: ScopeKindArg, scope_id: &str) -> Result<DeploymentPostureScope> {
    let parsed_uuid = Uuid::parse_str(scope_id)
        .with_context(|| format!("parse --scope-id `{scope_id}` as UUID"))?;
    let scope = match kind {
        ScopeKindArg::Account => DeploymentPostureScope::Account {
            account_id: AccountId::from(parsed_uuid),
        },
        ScopeKindArg::Project => DeploymentPostureScope::Project {
            project_id: ProjectId::from(parsed_uuid),
        },
        ScopeKindArg::Installation => DeploymentPostureScope::Installation {
            installation_id: InstallationId::from(parsed_uuid),
        },
    };
    Ok(scope)
}

fn parse_posture_value(raw: &str) -> Result<DeploymentPosture> {
    DeploymentPosture::from_wire_value(raw).ok_or_else(|| {
        anyhow::anyhow!(
            "error: unsupported_posture — {}",
            DeploymentPostureFailureReason::UnsupportedPosture.summary()
        )
    })
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

fn posture_error(err: SetDeploymentPostureError) -> anyhow::Error {
    match err {
        SetDeploymentPostureError::Contract { failure } => {
            anyhow::anyhow!("error: {} — {}", failure.reason.code(), failure.detail)
        }
        SetDeploymentPostureError::Store { source } => {
            anyhow::anyhow!("error: internal_error — {source}")
        }
        _ => anyhow::anyhow!("error: internal_error — unknown posture failure"),
    }
}

fn load_persisted_session_token() -> Result<SessionToken> {
    let path = session_path();
    let raw = fs::read_to_string(&path).map_err(|_| {
        let failure = missing_or_expired_session_failure();
        posture_error(failure)
    })?;
    let token = raw.trim();
    if token.is_empty() {
        let failure = missing_or_expired_session_failure();
        return Err(posture_error(failure));
    }
    Ok(SessionToken::from_secret(SecretString::from(
        token.to_owned(),
    )))
}

fn write_json_line(value: &serde_json::Value) -> Result<()> {
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    let encoded = serde_json::to_string(value).context("encode JSON output")?;
    writeln!(handle, "{encoded}").context("write JSON output to stdout")?;
    Ok(())
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
    fs::write(&path, token).with_context(|| format!("write session to {}", path.display()))?;
    Ok(())
}
