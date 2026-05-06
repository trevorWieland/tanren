//! Tanren scriptable command-line client — runtime library.
//!
//! R-0001 (sub-8) promotes the runtime out of `bin/tanren-cli/src/main.rs` per the thin-binary-crate
//! profile. The binary shrinks to a wiring shell that initializes tracing and calls [`run`].
//! The CLI receives bearer-mode `SessionView` responses from `tanren-app-services` (no cookie jar).

mod account;
mod project;

use std::io::Write;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tanren_app_services::{Handlers, Store};
use tanren_identity_policy::{AccountId, OrgId, ProjectId, ProviderConnectionId};
use uuid::Uuid;

/// Top-level CLI shape, renamed to `Config` per the thin-binary-crate convention.
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
        action: account::AccountAction,
    },
    /// Project flow: connect, list, disconnect, specs, dependencies.
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
enum ProjectAction {
    /// Connect a repository as a Tanren project.
    Connect {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Account id initiating the connection.
        #[arg(long)]
        account_id: String,
        /// Organization that will own the project.
        #[arg(long)]
        org_id: String,
        /// Human-readable name for the project.
        #[arg(long)]
        name: String,
        /// Source-control provider connection id.
        #[arg(long)]
        provider_connection_id: String,
        /// Repository resource identifier within the provider connection.
        #[arg(long)]
        resource_id: String,
    },
    /// List projects accessible to an account.
    List {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Account whose projects to list.
        #[arg(long)]
        account_id: String,
    },
    /// Disconnect a project from Tanren without modifying the repository.
    Disconnect {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Project id to disconnect.
        #[arg(long)]
        project_id: String,
        /// Account requesting the disconnect.
        #[arg(long)]
        account_id: String,
    },
    /// Reconnect a previously disconnected project.
    Reconnect {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Project id to reconnect.
        #[arg(long)]
        project_id: String,
        /// Account requesting the reconnect.
        #[arg(long)]
        account_id: String,
    },
    /// List specs attached to a project.
    Specs {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Project whose specs to list.
        #[arg(long)]
        project_id: String,
        /// Account requesting the action.
        #[arg(long)]
        account_id: String,
    },
    /// List cross-project dependency links for a project.
    Dependencies {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Project whose dependencies to list.
        #[arg(long)]
        project_id: String,
        /// Account requesting the action.
        #[arg(long)]
        account_id: String,
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
        Some(Command::Account { action }) => account::dispatch_account(action),
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

fn dispatch_project(action: ProjectAction) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;
    runtime.block_on(run_project(action))
}

async fn store_and_ids(
    database_url: &str,
    project_id: &str,
    account_id: &str,
) -> Result<(Store, ProjectId, AccountId)> {
    let store = Store::connect(database_url)
        .await
        .context("connect to store")?;
    let pid = ProjectId::new(parse_uuid(project_id, "project_id")?);
    let aid = AccountId::new(parse_uuid(account_id, "account_id")?);
    Ok((store, pid, aid))
}

async fn run_project(action: ProjectAction) -> Result<()> {
    let handlers = Handlers::new();
    match action {
        ProjectAction::Connect {
            database_url,
            account_id,
            org_id,
            name,
            provider_connection_id,
            resource_id,
        } => {
            let store = Store::connect(&database_url)
                .await
                .context("connect to store")?;
            let account_id = parse_uuid(&account_id, "account_id")?;
            let org_id = parse_uuid(&org_id, "org_id")?;
            let provider_connection_id =
                parse_uuid(&provider_connection_id, "provider_connection_id")?;
            project::connect_project(
                &handlers,
                &store,
                AccountId::new(account_id),
                OrgId::new(org_id),
                name,
                ProviderConnectionId::new(provider_connection_id),
                resource_id,
            )
            .await
        }
        ProjectAction::List {
            database_url,
            account_id,
        } => {
            let store = Store::connect(&database_url)
                .await
                .context("connect to store")?;
            let account_id = parse_uuid(&account_id, "account_id")?;
            project::list_projects(&handlers, &store, &database_url, AccountId::new(account_id))
                .await
        }
        ProjectAction::Disconnect {
            database_url,
            project_id,
            account_id,
        } => {
            let (store, pid, aid) = store_and_ids(&database_url, &project_id, &account_id).await?;
            project::disconnect_project(&handlers, &store, &database_url, pid, aid).await
        }
        ProjectAction::Reconnect {
            database_url,
            project_id,
            account_id,
        } => {
            let (store, pid, aid) = store_and_ids(&database_url, &project_id, &account_id).await?;
            project::reconnect_project(&handlers, &store, pid, aid).await
        }
        ProjectAction::Specs {
            database_url,
            project_id,
            account_id,
        } => {
            let (store, pid, aid) = store_and_ids(&database_url, &project_id, &account_id).await?;
            project::project_specs(&handlers, &store, aid, pid).await
        }
        ProjectAction::Dependencies {
            database_url,
            project_id,
            account_id,
        } => {
            let (store, pid, aid) = store_and_ids(&database_url, &project_id, &account_id).await?;
            project::project_dependencies(&handlers, &store, aid, pid).await
        }
    }
}

fn parse_uuid(raw: &str, field: &str) -> Result<Uuid> {
    raw.parse::<Uuid>()
        .with_context(|| format!("parse --{field} as UUID"))
}
