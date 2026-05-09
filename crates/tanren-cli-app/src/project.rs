use std::io::Write;

use anyhow::{Context, Result};
use clap::Subcommand;
use tanren_app_services::project::{
    ActiveProjectQuery, ConnectExistingRepositoryCommand, CreateNewProjectCommand,
    ListVisibleProjectsQuery,
};
use tanren_app_services::{AppServiceError, Handlers, Store};
use tanren_contract::{
    ActiveProjectRequest, ConnectProjectRepositoryRequest, CreateProjectRequest,
    ListVisibleProjectsRequest, ProjectView,
};
use tanren_identity_policy::{AccountId, DesignatedHost, RepositoryRef};
use tanren_provider_integrations::{SourceControlProvider, production_source_control_provider};
use uuid::Uuid;

/// Project setup and visibility subcommands.
#[derive(Debug, Subcommand)]
pub(super) enum ProjectAction {
    /// Connect an existing repository as a project.
    ConnectRepository {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Owning account id.
        #[arg(long)]
        owning_account_id: String,
        /// Repository identity (`owner/name`).
        #[arg(long)]
        repository: String,
        /// Whether to select this project as active.
        #[arg(long, default_value_t = true)]
        select_as_active: bool,
    },
    /// Create a repository at a designated host and register the project.
    Create {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Owning account id.
        #[arg(long)]
        owning_account_id: String,
        /// Repository identity (`owner/name`).
        #[arg(long)]
        repository: String,
        /// Designated host to create the repository at.
        #[arg(long)]
        designated_host: String,
        /// Whether to select this project as active.
        #[arg(long, default_value_t = true)]
        select_as_active: bool,
    },
    /// List visible projects for an account.
    List {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Owning account id.
        #[arg(long)]
        owning_account_id: String,
    },
    /// Show active-project metadata for an account.
    Active {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Owning account id.
        #[arg(long)]
        owning_account_id: String,
    },
}

pub(super) fn dispatch_project(action: ProjectAction) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;
    runtime.block_on(run_project(action))
}

async fn run_project(action: ProjectAction) -> Result<()> {
    let handlers = Handlers::new();
    let provider = production_source_control_provider();
    match action {
        ProjectAction::ConnectRepository {
            database_url,
            owning_account_id,
            repository,
            select_as_active,
        } => {
            run_connect_repository(
                &handlers,
                provider.as_ref(),
                &database_url,
                &owning_account_id,
                &repository,
                select_as_active,
            )
            .await?;
        }
        ProjectAction::Create {
            database_url,
            owning_account_id,
            repository,
            designated_host,
            select_as_active,
        } => {
            run_create_project(
                &handlers,
                provider.as_ref(),
                &database_url,
                &owning_account_id,
                &repository,
                &designated_host,
                select_as_active,
            )
            .await?;
        }
        ProjectAction::List {
            database_url,
            owning_account_id,
        } => run_list_projects(&handlers, &database_url, &owning_account_id).await?,
        ProjectAction::Active {
            database_url,
            owning_account_id,
        } => run_active_project(&handlers, &database_url, &owning_account_id).await?,
    }
    Ok(())
}

async fn run_connect_repository(
    handlers: &Handlers,
    provider: &dyn SourceControlProvider,
    database_url: &str,
    owning_account_id: &str,
    repository: &str,
    select_as_active: bool,
) -> Result<()> {
    let store = Store::connect(database_url)
        .await
        .context("connect to store")?;
    let owning_account_id = parse_account_id(owning_account_id)?;
    let repository = parse_repository_ref(repository)?;
    let response = handlers
        .connect_project_repository(
            &store,
            provider,
            ConnectExistingRepositoryCommand {
                actor_account_id: owning_account_id,
                request: ConnectProjectRepositoryRequest {
                    owning_account_id,
                    repository,
                    select_as_active,
                },
            },
        )
        .await
        .map_err(project_error)?;
    print_project_line(&response.project)?;
    Ok(())
}

async fn run_create_project(
    handlers: &Handlers,
    provider: &dyn SourceControlProvider,
    database_url: &str,
    owning_account_id: &str,
    repository: &str,
    designated_host: &str,
    select_as_active: bool,
) -> Result<()> {
    let store = Store::connect(database_url)
        .await
        .context("connect to store")?;
    let owning_account_id = parse_account_id(owning_account_id)?;
    let repository = parse_repository_ref(repository)?;
    let designated_host = parse_designated_host(designated_host)?;
    let response = handlers
        .create_project(
            &store,
            provider,
            CreateNewProjectCommand {
                actor_account_id: owning_account_id,
                request: CreateProjectRequest {
                    owning_account_id,
                    repository,
                    designated_host,
                    select_as_active,
                },
            },
        )
        .await
        .map_err(project_error)?;
    print_project_line(&response.project)?;
    Ok(())
}

async fn run_list_projects(
    handlers: &Handlers,
    database_url: &str,
    owning_account_id: &str,
) -> Result<()> {
    let store = Store::connect(database_url)
        .await
        .context("connect to store")?;
    let owning_account_id = parse_account_id(owning_account_id)?;
    let response = handlers
        .list_visible_projects(
            &store,
            ListVisibleProjectsQuery {
                actor_account_id: owning_account_id,
                request: ListVisibleProjectsRequest { owning_account_id },
            },
        )
        .await
        .map_err(project_error)?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(
        handle,
        "owning_account_id={} projects={}",
        response.owning_account_id,
        response.projects.len()
    )
    .context("write project list summary")?;
    for project in &response.projects {
        write_project_line(&mut handle, project).context("write project list entry")?;
    }
    Ok(())
}

async fn run_active_project(
    handlers: &Handlers,
    database_url: &str,
    owning_account_id: &str,
) -> Result<()> {
    let store = Store::connect(database_url)
        .await
        .context("connect to store")?;
    let owning_account_id = parse_account_id(owning_account_id)?;
    let response = handlers
        .active_project(
            &store,
            ActiveProjectQuery {
                actor_account_id: owning_account_id,
                request: ActiveProjectRequest { owning_account_id },
            },
        )
        .await
        .map_err(project_error)?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    match response.active_project.as_ref() {
        Some(project) => {
            writeln!(handle, "owning_account_id={}", response.owning_account_id)
                .context("write active project account id")?;
            write_project_line(&mut handle, project).context("write active project")?;
        }
        None => {
            writeln!(
                handle,
                "owning_account_id={} active_project=none",
                response.owning_account_id
            )
            .context("write empty active project report")?;
        }
    }
    Ok(())
}

fn parse_account_id(raw: &str) -> Result<AccountId> {
    let id = Uuid::parse_str(raw).context("parse --owning-account-id as uuid")?;
    Ok(AccountId::new(id))
}

fn parse_repository_ref(raw: &str) -> Result<RepositoryRef> {
    RepositoryRef::parse(raw).context("parse --repository as owner/name")
}

fn parse_designated_host(raw: &str) -> Result<DesignatedHost> {
    DesignatedHost::parse(raw).context("parse --designated-host as host key")
}

fn print_project_line(project: &ProjectView) -> Result<()> {
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    write_project_line(&mut handle, project).context("write project result")
}

fn write_project_line(mut out: impl Write, project: &ProjectView) -> Result<()> {
    writeln!(
        out,
        "project_id={} repository={} active={} specs={} milestones={} initiatives={}",
        project.id,
        project.repository.repository,
        project.selection.is_active,
        project.counts.specs,
        project.counts.milestones,
        project.counts.initiatives,
    )
    .context("write project line")
}

fn project_error(err: AppServiceError) -> anyhow::Error {
    match err {
        AppServiceError::Project(reason) => {
            anyhow::anyhow!("error: {} — {}", reason.code(), reason.summary())
        }
        AppServiceError::InvalidInput(message) => {
            anyhow::anyhow!("error: validation_failed — {message}")
        }
        AppServiceError::Store(err) => anyhow::anyhow!("error: internal_error — {err}"),
        _ => anyhow::anyhow!("error: internal_error — unknown app-service failure"),
    }
}
