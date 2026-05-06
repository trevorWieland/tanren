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

mod commands;
mod notifications;

use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

const SESSION_FILE_ENV: &str = "TANREN_SESSION_FILE";

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
    Account {
        #[command(subcommand)]
        action: AccountAction,
    },
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    Credential {
        #[command(subcommand)]
        action: CredentialAction,
    },
    Notification {
        #[command(subcommand)]
        action: NotificationAction,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum MigrateAction {
    Up {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum AccountAction {
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
}

#[derive(Debug, Subcommand)]
pub(crate) enum ConfigAction {
    User {
        #[command(subcommand)]
        action: UserConfigAction,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum UserConfigAction {
    List {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
    },
    Set {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long)]
        key: String,
        #[arg(long)]
        value: String,
    },
    Remove {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long)]
        key: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum CredentialAction {
    Add {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long)]
        kind: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        value: String,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        provider: Option<String>,
    },
    Update {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        value: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
    },
    List {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
    },
    Remove {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long)]
        id: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum NotificationAction {
    Preferences {
        #[command(subcommand)]
        action: NotificationPrefAction,
    },
    #[command(name = "org-override")]
    OrgOverride {
        #[command(subcommand)]
        action: NotificationOrgOverrideAction,
    },
    Route {
        #[command(subcommand)]
        action: NotificationRouteAction,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum NotificationPrefAction {
    Set {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long)]
        event_type: String,
        #[arg(long)]
        channels: String,
    },
    List {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum NotificationOrgOverrideAction {
    Set {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long)]
        org_id: String,
        #[arg(long)]
        event_type: String,
        #[arg(long)]
        channels: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum NotificationRouteAction {
    Evaluate {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long)]
        event_type: String,
        #[arg(long)]
        org_id: Option<String>,
    },
}

#[must_use]
pub fn run(config: Config) -> ExitCode {
    let result = match config.command {
        None | Some(Command::Health) => print_health(),
        Some(Command::Migrate {
            action: MigrateAction::Up { database_url },
        }) => run_migrate_up(&database_url),
        Some(Command::Account { action }) => commands::dispatch_account(action),
        Some(Command::Config {
            action: ConfigAction::User { action },
        }) => commands::dispatch_user_config(action),
        Some(Command::Credential { action }) => commands::dispatch_credential(action),
        Some(Command::Notification { action }) => notifications::dispatch_notification(action),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            let _ = writeln!(std::io::stderr().lock(), "{err}");
            ExitCode::from(1)
        }
    }
}

fn print_health() -> Result<()> {
    let r = tanren_app_services::Handlers::new().health(env!("CARGO_PKG_VERSION"));
    writeln!(
        std::io::stdout().lock(),
        "status={status} version={version} contract_version={contract}",
        status = r.status,
        version = r.version,
        contract = r.contract_version.value(),
    )
    .context("write health report")
}

fn run_migrate_up(database_url: &str) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;
    rt.block_on(async {
        tanren_app_services::Handlers::new()
            .migrate(database_url)
            .await
            .context("apply pending migrations")
    })?;
    writeln!(std::io::stdout().lock(), "migrations: applied").context("write migrate report")
}

pub(crate) fn session_path() -> PathBuf {
    if let Ok(v) = env::var(SESSION_FILE_ENV) {
        if !v.is_empty() {
            return PathBuf::from(v);
        }
    }
    let base = env::var("XDG_STATE_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map_or_else(
            || {
                env::var("HOME").ok().map_or_else(
                    || PathBuf::from("."),
                    |h| PathBuf::from(h).join(".local/state"),
                )
            },
            PathBuf::from,
        );
    base.join("tanren").join("session")
}

pub(crate) fn persist_session(token: &str) -> Result<()> {
    let path = session_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create session dir {}", parent.display()))?;
    }
    fs::write(&path, token).with_context(|| format!("write session to {}", path.display()))?;
    Ok(())
}
