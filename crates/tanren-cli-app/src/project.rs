use std::io::Write;

use anyhow::{Context, Result};
use clap::{Subcommand, ValueEnum};
use serde::Serialize;
use tanren_app_services::project::{
    ActiveProjectQuery, ConnectExistingRepositoryCommand, CreateNewProjectCommand,
    ListVisibleProjectsQuery,
};
use tanren_app_services::{AppServiceError, Handlers, Store};
use tanren_contract::{
    ActiveProjectRequest, ConnectProjectRepositoryRequest, CreateProjectRequest,
    ListVisibleProjectsRequest, ProjectCollectionView, ProjectPageRequest, ProjectView,
};
use tanren_provider_integrations::{SourceControlProvider, production_source_control_provider};
use tracing::error;

mod auth;
use auth::{
    parse_account_id, parse_designated_host, parse_repository_ref, resolve_actor_account_id,
};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(super) enum ProjectOutputMode {
    Text,
    Json,
}

#[derive(Debug, Serialize)]
struct ProjectFailureBody {
    code: String,
    summary: String,
}

impl ProjectFailureBody {
    fn validation(summary: impl Into<String>) -> Self {
        Self {
            code: "validation_failed".to_owned(),
            summary: summary.into(),
        }
    }

    fn internal(summary: impl Into<String>) -> Self {
        Self {
            code: "internal_error".to_owned(),
            summary: summary.into(),
        }
    }
}

type ProjectCommandResult<T> = std::result::Result<T, ProjectFailureBody>;

#[derive(Clone, Copy)]
struct ProjectRequestScope<'a> {
    database_url: &'a str,
    owning_account_id: &'a str,
    session_token_stdin: bool,
}

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
        /// Read session token from stdin instead of env/session file.
        #[arg(long, default_value_t = false)]
        session_token_stdin: bool,
        /// Repository identity (`owner/name`).
        #[arg(long)]
        repository: String,
        /// Whether to select this project as active.
        #[arg(long, default_value_t = true)]
        select_as_active: bool,
        /// Output mode.
        #[arg(long, value_enum, default_value_t = ProjectOutputMode::Text)]
        output: ProjectOutputMode,
    },
    /// Create a repository at a designated host and register the project.
    Create {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Owning account id.
        #[arg(long)]
        owning_account_id: String,
        /// Read session token from stdin instead of env/session file.
        #[arg(long, default_value_t = false)]
        session_token_stdin: bool,
        /// Repository identity (`owner/name`).
        #[arg(long)]
        repository: String,
        /// Designated host to create the repository at.
        #[arg(long)]
        designated_host: String,
        /// Whether to select this project as active.
        #[arg(long, default_value_t = true)]
        select_as_active: bool,
        /// Output mode.
        #[arg(long, value_enum, default_value_t = ProjectOutputMode::Text)]
        output: ProjectOutputMode,
    },
    /// List visible projects for an account.
    List {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Owning account id.
        #[arg(long)]
        owning_account_id: String,
        /// Read session token from stdin instead of env/session file.
        #[arg(long, default_value_t = false)]
        session_token_stdin: bool,
        /// Output mode.
        #[arg(long, value_enum, default_value_t = ProjectOutputMode::Text)]
        output: ProjectOutputMode,
    },
    /// Show active-project metadata for an account.
    Active {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Owning account id.
        #[arg(long)]
        owning_account_id: String,
        /// Read session token from stdin instead of env/session file.
        #[arg(long, default_value_t = false)]
        session_token_stdin: bool,
        /// Output mode.
        #[arg(long, value_enum, default_value_t = ProjectOutputMode::Text)]
        output: ProjectOutputMode,
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
            session_token_stdin,
            repository,
            select_as_active,
            output,
        } => {
            match run_connect_repository(
                &handlers,
                provider.as_ref(),
                &database_url,
                &owning_account_id,
                session_token_stdin,
                &repository,
                select_as_active,
            )
            .await
            {
                Ok(response) => emit_connect_repository_success(output, &response)?,
                Err(failure) => return emit_project_failure(output, &failure),
            }
        }
        ProjectAction::Create {
            database_url,
            owning_account_id,
            session_token_stdin,
            repository,
            designated_host,
            select_as_active,
            output,
        } => {
            match run_create_project(
                &handlers,
                provider.as_ref(),
                ProjectRequestScope {
                    database_url: &database_url,
                    owning_account_id: &owning_account_id,
                    session_token_stdin,
                },
                &repository,
                &designated_host,
                select_as_active,
            )
            .await
            {
                Ok(response) => emit_create_project_success(output, &response)?,
                Err(failure) => return emit_project_failure(output, &failure),
            }
        }
        ProjectAction::List {
            database_url,
            owning_account_id,
            session_token_stdin,
            output,
        } => match run_list_projects(
            &handlers,
            &database_url,
            &owning_account_id,
            session_token_stdin,
        )
        .await
        {
            Ok(response) => emit_list_projects_success(output, &response)?,
            Err(failure) => return emit_project_failure(output, &failure),
        },
        ProjectAction::Active {
            database_url,
            owning_account_id,
            session_token_stdin,
            output,
        } => match run_active_project(
            &handlers,
            &database_url,
            &owning_account_id,
            session_token_stdin,
        )
        .await
        {
            Ok(response) => emit_active_project_success(output, &response)?,
            Err(failure) => return emit_project_failure(output, &failure),
        },
    }
    Ok(())
}

async fn run_connect_repository(
    handlers: &Handlers,
    provider: &dyn SourceControlProvider,
    database_url: &str,
    owning_account_id: &str,
    session_token_stdin: bool,
    repository: &str,
    select_as_active: bool,
) -> ProjectCommandResult<tanren_contract::ConnectProjectRepositoryResponse> {
    let store = connect_store(database_url).await?;
    let owning_account_id = parse_account_id(owning_account_id)?;
    let actor_account_id = resolve_actor_account_id(&store, session_token_stdin).await?;
    let repository = parse_repository_ref(repository)?;
    handlers
        .connect_project_repository(
            &store,
            provider,
            ConnectExistingRepositoryCommand {
                actor_account_id,
                request: ConnectProjectRepositoryRequest {
                    owning_account_id,
                    repository,
                    select_as_active,
                },
            },
        )
        .await
        .map_err(project_error)
}

async fn run_create_project(
    handlers: &Handlers,
    provider: &dyn SourceControlProvider,
    scope: ProjectRequestScope<'_>,
    repository: &str,
    designated_host: &str,
    select_as_active: bool,
) -> ProjectCommandResult<tanren_contract::CreateProjectResponse> {
    let store = connect_store(scope.database_url).await?;
    let owning_account_id = parse_account_id(scope.owning_account_id)?;
    let actor_account_id = resolve_actor_account_id(&store, scope.session_token_stdin).await?;
    let repository = parse_repository_ref(repository)?;
    let designated_host = parse_designated_host(designated_host)?;
    handlers
        .create_project(
            &store,
            provider,
            CreateNewProjectCommand {
                actor_account_id,
                request: CreateProjectRequest {
                    owning_account_id,
                    repository,
                    designated_host,
                    select_as_active,
                },
            },
        )
        .await
        .map_err(project_error)
}

async fn run_list_projects(
    handlers: &Handlers,
    database_url: &str,
    owning_account_id: &str,
    session_token_stdin: bool,
) -> ProjectCommandResult<ProjectCollectionView> {
    let store = connect_store(database_url).await?;
    let owning_account_id = parse_account_id(owning_account_id)?;
    let actor_account_id = resolve_actor_account_id(&store, session_token_stdin).await?;
    handlers
        .list_visible_projects(
            &store,
            ListVisibleProjectsQuery {
                actor_account_id,
                request: ListVisibleProjectsRequest {
                    owning_account_id,
                    page: ProjectPageRequest::default(),
                },
            },
        )
        .await
        .map_err(project_error)
}

async fn run_active_project(
    handlers: &Handlers,
    database_url: &str,
    owning_account_id: &str,
    session_token_stdin: bool,
) -> ProjectCommandResult<tanren_contract::ActiveProjectView> {
    let store = connect_store(database_url).await?;
    let owning_account_id = parse_account_id(owning_account_id)?;
    let actor_account_id = resolve_actor_account_id(&store, session_token_stdin).await?;
    handlers
        .active_project(
            &store,
            ActiveProjectQuery {
                actor_account_id,
                request: ActiveProjectRequest { owning_account_id },
            },
        )
        .await
        .map_err(project_error)
}

fn emit_connect_repository_success(
    mode: ProjectOutputMode,
    response: &tanren_contract::ConnectProjectRepositoryResponse,
) -> Result<()> {
    match mode {
        ProjectOutputMode::Text => print_project_line(&response.project),
        ProjectOutputMode::Json => write_json(response),
    }
}

fn emit_create_project_success(
    mode: ProjectOutputMode,
    response: &tanren_contract::CreateProjectResponse,
) -> Result<()> {
    match mode {
        ProjectOutputMode::Text => print_project_line(&response.project),
        ProjectOutputMode::Json => write_json(response),
    }
}

fn emit_list_projects_success(
    mode: ProjectOutputMode,
    response: &ProjectCollectionView,
) -> Result<()> {
    match mode {
        ProjectOutputMode::Text => print_project_collection(response),
        ProjectOutputMode::Json => write_json(response),
    }
}

fn emit_active_project_success(
    mode: ProjectOutputMode,
    response: &tanren_contract::ActiveProjectView,
) -> Result<()> {
    match mode {
        ProjectOutputMode::Text => print_active_project(response),
        ProjectOutputMode::Json => write_json(response),
    }
}

fn emit_project_failure(mode: ProjectOutputMode, failure: &ProjectFailureBody) -> Result<()> {
    if matches!(mode, ProjectOutputMode::Json) {
        write_json(&failure).context("write project failure json")?;
    }
    Err(anyhow::anyhow!(
        "error: {} — {}",
        failure.code,
        failure.summary
    ))
}

fn print_project_collection(response: &ProjectCollectionView) -> Result<()> {
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

fn print_active_project(response: &tanren_contract::ActiveProjectView) -> Result<()> {
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

async fn connect_store(database_url: &str) -> ProjectCommandResult<Store> {
    Store::connect(database_url).await.map_err(|err| {
        error!(error = ?err, "project command failed to connect store");
        ProjectFailureBody::internal(
            "Tanren encountered an internal error while processing the project request.",
        )
    })
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

fn project_error(err: AppServiceError) -> ProjectFailureBody {
    match err {
        AppServiceError::Project(reason) => ProjectFailureBody {
            code: reason.code().to_owned(),
            summary: reason.summary().to_owned(),
        },
        AppServiceError::InvalidInput(message) => ProjectFailureBody::validation(message),
        AppServiceError::Store(err) => {
            error!(error = ?err, "project command store failure");
            ProjectFailureBody::internal(
                "Tanren encountered an internal error while processing the project request.",
            )
        }
        other => {
            error!(error = ?other, "project command unexpected app-service failure");
            ProjectFailureBody::internal(
                "Tanren encountered an internal error while processing the project request.",
            )
        }
    }
}

fn write_json<T: Serialize>(value: &T) -> Result<()> {
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    serde_json::to_writer(&mut handle, value).context("serialize project output as json")?;
    writeln!(handle).context("terminate project json output line")
}
