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
use clap::{Parser, Subcommand};
use secrecy::SecretString;
use serde::Deserialize;
use tanren_app_services::install::evaluate_drift;
use tanren_app_services::{AppServiceError, Handlers, Store};
use tanren_contract::{
    AcceptInvitationRequest, DriftPolicy, InstallDriftResponse, PreservationPolicy, SignInRequest,
    SignUpRequest,
};
use tanren_identity_policy::{Email, InvitationToken};

const SESSION_FILE_ENV: &str = "TANREN_SESSION_FILE";
const API_URL_ENV: &str = "TANREN_API_URL";
const DEFAULT_API_URL: &str = "http://localhost:8080";

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
    /// Install and drift-check Tanren assets.
    Install {
        #[command(subcommand)]
        action: InstallAction,
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

#[derive(Debug, Subcommand)]
enum InstallAction {
    /// Check installed assets for drift without modifying the repository.
    Drift {
        /// Project identifier (UUID) of the installed repository.
        #[arg(long, group = "target")]
        project: Option<String>,
        /// Local repository path for direct drift check (no API required).
        #[arg(long, group = "target")]
        repo: Option<PathBuf>,
        /// Output format.
        #[arg(long, value_parser = validate_report_kind, default_value = "json")]
        format: ReportKind,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReportKind {
    Json,
}

fn validate_report_kind(s: &str) -> Result<ReportKind, String> {
    match s {
        "json" => Ok(ReportKind::Json),
        other => Err(format!("unsupported format: {other}")),
    }
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
        Some(Command::Install { action }) => dispatch_install(action),
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
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            writeln!(
                handle,
                "account_id={id} session={token}",
                id = response.account.id,
                token = response.session.token.expose_secret(),
            )
            .context("write sign-in result")?;
        }
    }
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

fn dispatch_install(action: InstallAction) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;
    match action {
        InstallAction::Drift {
            project: Some(project),
            format,
            ..
        } => runtime.block_on(run_install_drift(&project, format)),
        InstallAction::Drift {
            repo: Some(repo),
            format,
            ..
        } => run_local_drift(&repo, format),
        InstallAction::Drift {
            project: None,
            repo: None,
            ..
        } => anyhow::bail!(
            "error: validation_failed — exactly one of --project or --repo is required"
        ),
    }
}

fn run_local_drift(repo: &std::path::Path, kind: ReportKind) -> Result<()> {
    let result = evaluate_drift(
        repo,
        DriftPolicy::AllAssets,
        PreservationPolicy::AcceptUserEdits,
    )
    .map_err(|err| anyhow::anyhow!("error: internal_error — {err}"))?;
    let report = InstallDriftResponse {
        has_drift: result.has_drift,
        entries: result.entries,
        config_source: tanren_contract::DriftConfigSource {
            drift_policy: DriftPolicy::AllAssets,
            preservation_policy: PreservationPolicy::AcceptUserEdits,
        },
    };
    match kind {
        ReportKind::Json => {
            let json = serde_json::to_string(&report).context("serialize drift report")?;
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            writeln!(handle, "{json}").context("write drift report")?;
        }
    }
    if report.has_drift {
        anyhow::bail!("drift detected");
    }
    Ok(())
}

async fn run_install_drift(project: &str, kind: ReportKind) -> Result<()> {
    let api_url = env::var(API_URL_ENV).unwrap_or_else(|_| DEFAULT_API_URL.to_owned());
    let url = format!("{api_url}/projects/{project}/install/drift");

    let response = reqwest::get(&url)
        .await
        .with_context(|| format!("request drift check from {url}"))?;

    let status = response.status();
    if status.is_success() {
        let drift: InstallDriftResponse = response.json().await.context("parse drift response")?;
        match kind {
            ReportKind::Json => {
                let json = serde_json::to_string(&drift).context("serialize drift report")?;
                let stdout = std::io::stdout();
                let mut handle = stdout.lock();
                writeln!(handle, "{json}").context("write drift report")?;
            }
        }
        if drift.has_drift {
            anyhow::bail!("drift detected");
        }
        Ok(())
    } else {
        let body: DriftApiError = response.json().await.context("parse error response")?;
        match body.code.as_str() {
            "validation_failed" => {
                anyhow::bail!("error: validation_failed — {}", body.summary)
            }
            "project_not_found" => {
                anyhow::bail!("error: project_not_found — {}", body.summary)
            }
            _ => {
                anyhow::bail!("error: internal_error — {}", body.summary)
            }
        }
    }
}

#[derive(Deserialize)]
struct DriftApiError {
    code: String,
    summary: String,
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
