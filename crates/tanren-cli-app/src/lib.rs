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
use std::future::Future;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use secrecy::SecretString;
use tanren_app_services::{AppServiceError, Handlers, Store};
use tanren_contract::{AcceptInvitationRequest, SignInRequest, SignUpRequest};
use tanren_identity_policy::{AccountId, Email, InvitationToken, ProjectId, SpecId};

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
    /// Project flow: list, switch, scoped-views, attention-spec drill-down.
    Project {
        #[command(subcommand)]
        action: ProjectAction,
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
enum ProjectAction {
    /// List all projects in the active account with attention markers.
    List {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long)]
        account_id: String,
    },
    /// Switch the active project for the given account.
    Switch {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long)]
        account_id: String,
        #[arg(long)]
        project_id: String,
    },
    /// Show specs, loops, and milestones for the active project.
    ScopedViews {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long)]
        account_id: String,
    },
    /// Drill down into a spec that needs attention.
    AttentionSpec {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long)]
        account_id: String,
        #[arg(long)]
        project_id: String,
        #[arg(long)]
        spec_id: String,
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
        Some(Command::Project { action }) => dispatch_project(action),
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

fn blocking<F, T>(fut: F) -> Result<T>
where
    F: Future<Output = Result<T>>,
{
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;
    rt.block_on(fut)
}

fn run_migrate_up(database_url: &str) -> Result<()> {
    blocking(async {
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
    blocking(run_account(action))
}

fn dispatch_project(action: ProjectAction) -> Result<()> {
    blocking(run_project(action))
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
                    let response = handle(handlers.sign_up(
                        &store,
                        SignUpRequest {
                            email,
                            password,
                            display_name,
                        },
                    ))
                    .await?;
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
                    let response = handle(handlers.accept_invitation(
                        &store,
                        AcceptInvitationRequest {
                            invitation_token,
                            email,
                            password,
                            display_name,
                        },
                    ))
                    .await?;
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
            let response =
                handle(handlers.sign_in(&store, SignInRequest { email, password })).await?;
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

async fn connect_store(url: &str) -> Result<Store> {
    Store::connect(url).await.context("connect to store")
}

async fn handle<F, T>(fut: F) -> Result<T>
where
    F: Future<Output = std::result::Result<T, AppServiceError>>,
{
    fut.await.map_err(failure)
}

async fn run_project(action: ProjectAction) -> Result<()> {
    let handlers = Handlers::new();
    let stdout = std::io::stdout();
    match action {
        ProjectAction::List {
            database_url,
            account_id,
        } => {
            let store = connect_store(&database_url).await?;
            let aid = parse_account_id(&account_id)?;
            let projects = handle(handlers.list_projects(&store, aid)).await?;
            let mut handle = stdout.lock();
            for p in &projects {
                let att = if p.needs_attention { "true" } else { "false" };
                writeln!(
                    handle,
                    "id={} name={} state={:?} needs_attention={} created_at={}",
                    p.id, p.name, p.state, att, p.created_at
                )
                .context("write project row")?;
                for s in &p.attention_specs {
                    writeln!(
                        handle,
                        "  attention_spec: id={} name={} reason={}",
                        s.id, s.name, s.reason
                    )
                    .context("write attention-spec row")?;
                }
            }
        }
        ProjectAction::Switch {
            database_url,
            account_id,
            project_id,
        } => {
            let store = connect_store(&database_url).await?;
            let aid = parse_account_id(&account_id)?;
            let pid = parse_project_id(&project_id)?;
            let resp = handle(handlers.switch_active_project(&store, aid, pid)).await?;
            let mut handle = stdout.lock();
            writeln!(
                handle,
                "project_id={} name={} state={:?} needs_attention={} specs={} loops={} milestones={}",
                resp.project.id, resp.project.name, resp.project.state, resp.project.needs_attention,
                resp.scoped.specs.len(), resp.scoped.loops.len(), resp.scoped.milestones.len(),
            ).context("write switch result")?;
        }
        ProjectAction::ScopedViews {
            database_url,
            account_id,
        } => {
            let store = connect_store(&database_url).await?;
            let aid = parse_account_id(&account_id)?;
            let views = handle(handlers.project_scoped_views(&store, aid)).await?;
            let mut handle = stdout.lock();
            writeln!(
                handle,
                "project_id={} specs=[{}] loops=[{}] milestones=[{}]",
                views.project_id,
                format_ids(&views.specs),
                format_ids(&views.loops),
                format_ids(&views.milestones),
            )
            .context("write scoped-views result")?;
        }
        ProjectAction::AttentionSpec {
            database_url,
            account_id,
            project_id,
            spec_id,
        } => {
            let store = connect_store(&database_url).await?;
            let aid = parse_account_id(&account_id)?;
            let pid = parse_project_id(&project_id)?;
            let sid = parse_spec_id(&spec_id)?;
            let spec = handle(handlers.attention_spec(&store, aid, pid, sid)).await?;
            let mut handle = stdout.lock();
            writeln!(
                handle,
                "spec_id={} name={} reason={}",
                spec.id, spec.name, spec.reason
            )
            .context("write attention-spec result")?;
        }
    }
    Ok(())
}

fn parse_account_id(raw: &str) -> Result<AccountId> {
    raw.parse::<uuid::Uuid>()
        .map(AccountId::new)
        .with_context(|| format!("parse account id: {raw}"))
}

fn parse_project_id(raw: &str) -> Result<ProjectId> {
    raw.parse::<uuid::Uuid>()
        .map(ProjectId::new)
        .with_context(|| format!("parse project id: {raw}"))
}

fn parse_spec_id(raw: &str) -> Result<SpecId> {
    raw.parse::<uuid::Uuid>()
        .map(SpecId::new)
        .with_context(|| format!("parse spec id: {raw}"))
}

fn format_ids<T: std::fmt::Display>(ids: &[T]) -> String {
    ids.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn failure(err: AppServiceError) -> anyhow::Error {
    match err {
        AppServiceError::Account(r) => failure_msg(r.code(), r.summary()),
        AppServiceError::Project(r) => failure_msg(r.code(), r.summary()),
        AppServiceError::InvalidInput(m) => failure_msg("validation_failed", &m),
        AppServiceError::Store(e) => failure_msg("internal_error", &e.to_string()),
        _ => failure_msg("internal_error", "unknown app-service failure"),
    }
}

fn failure_msg(code: &str, detail: &str) -> anyhow::Error {
    anyhow::anyhow!("error: {code} — {detail}")
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
