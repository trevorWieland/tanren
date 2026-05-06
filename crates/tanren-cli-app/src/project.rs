//! CLI project-flow commands, dispatch, and helper types.
//!
//! Extracted from `lib.rs` to keep that file under the workspace 500-line
//! budget.  Owns [`ProjectAction`] (the `clap` sub-enum), the async
//! dispatch path, and the `parse_account_id` helper.

use std::io::Write;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Subcommand;
use tanren_app_services::{AppServiceError, Handlers, SourceControlProvider, Store};
use tanren_contract::{
    ConnectProjectRequest, CreateProjectRequest, ProjectFailureReason, ProjectView,
};
use tanren_identity_policy::AccountId;
use tanren_provider_integrations::{NullProviderRegistry, ProviderError, ProviderRegistry};
use uuid::Uuid;

use super::service_error;

#[derive(Debug, Subcommand)]
pub(crate) enum ProjectAction {
    /// Connect an existing repository as a new project (B-0025).
    Connect {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Account ID (from account create / sign-in output).
        #[arg(long)]
        account_id: String,
        /// Human-readable project name.
        #[arg(long)]
        name: String,
        /// Fully-qualified URL of the existing repository.
        #[arg(long)]
        repository_url: String,
    },
    /// Create a new project and its backing repository (B-0026).
    Create {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Account ID (from account create / sign-in output).
        #[arg(long)]
        account_id: String,
        /// Human-readable project name.
        #[arg(long)]
        name: String,
        /// SCM host where the repository will be created.
        #[arg(long)]
        provider_host: String,
    },
    /// Print the caller's currently active project.
    Active {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Account ID (from account create / sign-in output).
        #[arg(long)]
        account_id: String,
    },
}

pub(crate) fn dispatch_project(action: ProjectAction) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;
    runtime.block_on(run_project(action))
}

fn resolve_provider() -> Result<Arc<dyn SourceControlProvider>> {
    NullProviderRegistry.resolve().map_err(|e| match e {
        ProviderError::NotConfigured => service_error(AppServiceError::Project(
            ProjectFailureReason::ProviderNotConfigured,
        )),
        _ => service_error(AppServiceError::Project(
            ProjectFailureReason::ProviderFailure,
        )),
    })
}

async fn run_project(action: ProjectAction) -> Result<()> {
    let handlers = Handlers::new();
    match action {
        ProjectAction::Connect {
            database_url,
            account_id,
            name,
            repository_url,
        } => {
            let store = Store::connect(&database_url)
                .await
                .context("connect to store")?;
            let aid = parse_account_id(&account_id)?;
            let scm = resolve_provider()?;
            let response = handlers
                .connect_project(
                    &store,
                    scm.as_ref(),
                    aid,
                    ConnectProjectRequest {
                        name,
                        repository_url,
                        org: None,
                    },
                )
                .await
                .map_err(service_error)?;
            write_project_line(&response, "active=true")?;
        }
        ProjectAction::Create {
            database_url,
            account_id,
            name,
            provider_host,
        } => {
            let store = Store::connect(&database_url)
                .await
                .context("connect to store")?;
            let aid = parse_account_id(&account_id)?;
            let scm = resolve_provider()?;
            let response = handlers
                .create_project(
                    &store,
                    scm.as_ref(),
                    aid,
                    CreateProjectRequest {
                        name,
                        provider_host,
                        org: None,
                    },
                )
                .await
                .map_err(service_error)?;
            write_project_line(&response, "active=true")?;
        }
        ProjectAction::Active {
            database_url,
            account_id,
        } => {
            let store = Store::connect(&database_url)
                .await
                .context("connect to store")?;
            let aid = parse_account_id(&account_id)?;
            match handlers
                .active_project(&store, aid)
                .await
                .map_err(service_error)?
            {
                None => {
                    let stdout = std::io::stdout();
                    let mut handle = stdout.lock();
                    writeln!(handle, "active=none").context("write active-project result")?;
                }
                Some(view) => {
                    write_project_line(
                        &view.project,
                        &format!("activated_at={}", view.activated_at),
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn write_project_line(p: &ProjectView, suffix: &str) -> Result<()> {
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(handle,
        "project_id={} repository_id={} repository_url={} {suffix} specs={} milestones={} initiatives={}",
        p.id, p.repository.id, p.repository.url,
        p.content_counts.specs, p.content_counts.milestones, p.content_counts.initiatives)
    .context("write project result")
}

pub(crate) fn parse_account_id(raw: &str) -> Result<AccountId> {
    let uuid =
        Uuid::parse_str(raw).with_context(|| format!("parse --account-id as UUID: {raw}"))?;
    Ok(AccountId::new(uuid))
}
