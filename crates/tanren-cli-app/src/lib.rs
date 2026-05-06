//! Tanren scriptable command-line client — runtime library.
//!
//! Promoted from `bin/tanren-cli/src/main.rs` per the thin-binary-crate
//! profile. The binary shrinks to a wiring shell that initializes tracing
//! and calls [`run`]; everything below lives here so the BDD harness can
//! depend on it without spinning up a child process. The CLI receives
//! bearer-mode `SessionView` responses (no cookie jar).

use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};
use secrecy::SecretString;
use tanren_app_services::{AccountStore, AppServiceError, Handlers, Store};
use tanren_contract::{
    AcceptInvitationRequest, AccountFailureReason, JoinOrganizationRequest,
    LeaveOrganizationRequest, RemoveMemberRequest, SignInRequest, SignUpRequest,
};
use tanren_identity_policy::{AccountId, Email, InvitationToken, SessionToken};
use uuid::Uuid;

const SESSION_FILE_ENV: &str = "TANREN_SESSION_FILE";

/// Top-level CLI shape, renamed to `Config` per the thin-binary-crate
/// convention.
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
    Health,
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
    Create {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long)]
        identifier: String,
        #[arg(long)]
        password: String,
        #[arg(long, default_value_t = String::from("Tanren user"))]
        display_name: String,
        #[arg(long)]
        invitation: Option<String>,
    },
    SignIn {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long)]
        identifier: String,
        #[arg(long)]
        password: String,
    },
    Join {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long)]
        invitation: String,
    },
    Leave {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long)]
        org: String,
        #[arg(long)]
        acknowledge_in_flight_work: bool,
    },
    /// Remove another account from an organization (admin-initiated).
    RemoveMember {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long)]
        org: String,
        #[arg(long)]
        account: String,
        #[arg(long)]
        acknowledge_in_flight_work: bool,
    },
}

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
            run_create(
                &handlers,
                &database_url,
                &identifier,
                &password,
                &display_name,
                invitation,
            )
            .await?;
        }
        AccountAction::SignIn {
            database_url,
            identifier,
            password,
        } => run_sign_in(&handlers, &database_url, &identifier, &password).await?,
        AccountAction::Join {
            database_url,
            invitation,
        } => run_join(&handlers, &database_url, &invitation).await?,
        AccountAction::Leave {
            database_url,
            org,
            acknowledge_in_flight_work,
        } => run_leave(&handlers, &database_url, &org, acknowledge_in_flight_work).await?,
        AccountAction::RemoveMember {
            database_url,
            org,
            account,
            acknowledge_in_flight_work,
        } => {
            run_remove_member(
                &handlers,
                &database_url,
                &org,
                &account,
                acknowledge_in_flight_work,
            )
            .await?;
        }
    }
    Ok(())
}

async fn run_create(
    handlers: &Handlers,
    database_url: &str,
    identifier: &str,
    password: &str,
    display_name: &str,
    invitation: Option<String>,
) -> Result<()> {
    let store = Store::connect(database_url)
        .await
        .context("connect to store")?;
    let email = Email::parse(identifier).context("parse --identifier as email")?;
    let password = SecretString::from(password.to_owned());
    match invitation {
        None => {
            let resp = handlers
                .sign_up(
                    &store,
                    SignUpRequest {
                        email,
                        password,
                        display_name: display_name.to_owned(),
                    },
                )
                .await
                .map_err(account_error)?;
            write_account_session(&resp.account.id, resp.session.token.expose_secret())?;
        }
        Some(token) => {
            let inv =
                InvitationToken::parse(&token).context("parse --invitation as invitation token")?;
            let resp = handlers
                .accept_invitation(
                    &store,
                    AcceptInvitationRequest {
                        invitation_token: inv,
                        email,
                        password,
                        display_name: display_name.to_owned(),
                    },
                )
                .await
                .map_err(account_error)?;
            persist_session(resp.session.token.expose_secret())?;
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            writeln!(
                handle,
                "account_id={} session={} joined_org={}",
                resp.account.id,
                resp.session.token.expose_secret(),
                resp.joined_org,
            )
            .context("write invitation-acceptance result")?;
        }
    }
    Ok(())
}

async fn run_sign_in(
    handlers: &Handlers,
    database_url: &str,
    identifier: &str,
    password: &str,
) -> Result<()> {
    let store = Store::connect(database_url)
        .await
        .context("connect to store")?;
    let email = Email::parse(identifier).context("parse --identifier as email")?;
    let password = SecretString::from(password.to_owned());
    let resp = handlers
        .sign_in(&store, SignInRequest { email, password })
        .await
        .map_err(account_error)?;
    write_account_session(&resp.account.id, resp.session.token.expose_secret())?;
    Ok(())
}

fn write_account_session(account_id: &AccountId, token: &str) -> Result<()> {
    persist_session(token)?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(handle, "account_id={account_id} session={token}").context("write account result")?;
    Ok(())
}

async fn run_join(handlers: &Handlers, database_url: &str, invitation: &str) -> Result<()> {
    let store = Store::connect(database_url)
        .await
        .context("connect to store")?;
    let session_token = load_session()?;
    let account_id = resolve_account_id(&store, &session_token).await?;
    let invitation_token =
        InvitationToken::parse(invitation).context("parse --invitation as invitation token")?;
    let response = handlers
        .join_organization(
            &store,
            account_id,
            JoinOrganizationRequest { invitation_token },
        )
        .await
        .map_err(account_error)?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(
        handle,
        "joined_org={org} permissions={perms} project_access=[]",
        org = response.joined_org,
        perms = response.membership_permissions,
    )
    .context("write join-organization result")?;
    Ok(())
}

async fn run_leave(
    handlers: &Handlers,
    database_url: &str,
    org: &str,
    acknowledge_in_flight_work: bool,
) -> Result<()> {
    let store = Store::connect(database_url)
        .await
        .context("connect to store")?;
    let session_token = load_session()?;
    let account_id = resolve_account_id(&store, &session_token).await?;
    let org_id = parse_uuid(org, "org")?;
    let response = handlers
        .leave_organization(
            &store,
            account_id,
            LeaveOrganizationRequest { org_id },
            acknowledge_in_flight_work,
        )
        .await
        .map_err(account_error)?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    write_departure_result(&mut handle, &response)
}

async fn run_remove_member(
    handlers: &Handlers,
    database_url: &str,
    org: &str,
    account: &str,
    acknowledge_in_flight_work: bool,
) -> Result<()> {
    let store = Store::connect(database_url)
        .await
        .context("connect to store")?;
    let session_token = load_session()?;
    let actor_account_id = resolve_account_id(&store, &session_token).await?;
    let org_id = parse_uuid(org, "org")?;
    let member_account_id = parse_uuid(account, "account")?;
    let response = handlers
        .remove_member(
            &store,
            actor_account_id,
            RemoveMemberRequest {
                org_id,
                member_account_id,
            },
            acknowledge_in_flight_work,
        )
        .await
        .map_err(account_error)?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    write_departure_result(&mut handle, &response)
}

fn parse_uuid<T>(raw: &str, label: &str) -> Result<T>
where
    T: From<Uuid>,
{
    let uuid = Uuid::parse_str(raw).with_context(|| format!("parse --{label} as UUID"))?;
    Ok(T::from(uuid))
}

fn write_departure_result(
    handle: &mut std::io::StdoutLock<'_>,
    response: &tanren_contract::MembershipDepartureResponse,
) -> Result<()> {
    let departed = response
        .departed_org
        .map_or_else(|| "none".into(), |id| id.to_string());
    writeln!(
        handle,
        "completed={} in_flight_work={} departed_org={departed}",
        response.completed,
        response.in_flight_work.len(),
    )
    .context("write departure result")
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

fn persist_session(token: &str) -> Result<()> {
    let path = session_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create session dir {}", parent.display()))?;
    }
    fs::write(&path, token).with_context(|| format!("write session to {}", path.display()))?;
    Ok(())
}

fn load_session() -> Result<SessionToken> {
    let path = session_path();
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("read session from {}", path.display()))?;
    Ok(SessionToken::from_secret(SecretString::from(raw)))
}

async fn resolve_account_id(store: &Store, token: &SessionToken) -> Result<AccountId> {
    let session = store
        .find_session_by_token(token)
        .await
        .context("lookup session")?
        .ok_or_else(|| {
            account_error(AppServiceError::Account(
                AccountFailureReason::Unauthenticated,
            ))
        })?;
    if session.expires_at <= Utc::now() {
        return Err(account_error(AppServiceError::Account(
            AccountFailureReason::Unauthenticated,
        )));
    }
    Ok(session.account_id)
}
