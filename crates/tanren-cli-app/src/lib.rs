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
use tanren_app_services::{AppServiceError, Handlers, MyPermissionsContext, Store};
use tanren_contract::{
    AcceptInvitationRequest, MyPermissionEntry, MyPermissionsRequest, MyPermissionsResponse,
    SignInRequest, SignUpRequest,
};
use tanren_identity_policy::{AccountId, Email, InvitationToken};
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

async fn run_account_my_permissions(
    handlers: &Handlers,
    database_url: &str,
    target_account_id: Option<String>,
) -> Result<()> {
    let store = Store::connect(database_url)
        .await
        .context("connect to store")?;
    let session_account_id = read_session_account_id()?;
    let requested_account_id = target_account_id
        .map(|raw| parse_account_id(&raw, "--target-account-id"))
        .transpose()?
        .unwrap_or(session_account_id);
    let response = handlers
        .my_permissions(
            &store,
            MyPermissionsContext {
                session_account_id,
                requested_account_id,
            },
            MyPermissionsRequest,
        )
        .await
        .map_err(account_error)?;
    print_permissions(&response)?;
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
        AppServiceError::Permissions(reason) => {
            anyhow::anyhow!("error: {} — {}", reason.code(), reason.summary())
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

fn persist_session(account_id: AccountId, token: &str) -> Result<()> {
    let path = session_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create session dir {}", parent.display()))?;
    }
    let content = format!("version=1\naccount_id={account_id}\ntoken={token}\n");
    fs::write(&path, content).with_context(|| format!("write session to {}", path.display()))?;
    Ok(())
}

fn read_session_account_id() -> Result<AccountId> {
    let path = session_path();
    let content = fs::read_to_string(&path)
        .with_context(|| format!("read session from {}", path.display()))?;
    for line in content.lines() {
        if let Some(raw) = line.strip_prefix("account_id=") {
            return parse_account_id(raw.trim(), "persisted session account_id");
        }
    }
    anyhow::bail!(
        "error: auth_required — session is missing account_id; sign in again to refresh {}",
        path.display()
    )
}

fn print_permissions(response: &MyPermissionsResponse) -> Result<()> {
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();

    if response.organizations.is_empty() && response.projects.is_empty() {
        writeln!(handle, "permissions=none").context("write permissions result")?;
        return Ok(());
    }

    for organization in &response.organizations {
        for permission in &organization.permissions {
            write_permission_row(
                &mut handle,
                "organization",
                &organization.org_id.to_string(),
                permission,
            )
            .context("write organization permission row")?;
        }
    }
    for project in &response.projects {
        for permission in &project.permissions {
            write_permission_row(
                &mut handle,
                "project",
                &project.project_id.to_string(),
                permission,
            )
            .context("write project permission row")?;
        }
    }
    Ok(())
}

fn write_permission_row(
    handle: &mut impl Write,
    scope: &str,
    scope_id: &str,
    permission: &MyPermissionEntry,
) -> Result<()> {
    let (constraint_reason, constraint_source) = permission.policy_constraint.as_ref().map_or_else(
        || ("none".to_owned(), "none".to_owned()),
        |constraint| {
            (
                format!("{:?}", constraint.reason),
                format!("{:?}", constraint.source),
            )
        },
    );

    writeln!(
        handle,
        "scope={scope} scope_id={scope_id} permission={permission_name:?} effective_state={effective_state:?} source={grant_source:?} constraint_reason={constraint_reason} constraint_source={constraint_source}",
        permission_name = permission.permission,
        effective_state = permission.effective_state,
        grant_source = permission.grant_source,
    )
    .context("write permission row")
}

fn parse_account_id(raw: &str, context_label: &str) -> Result<AccountId> {
    let parsed = Uuid::parse_str(raw)
        .with_context(|| format!("parse {context_label} as uuid-form account id"))?;
    Ok(AccountId::new(parsed))
}
