//! Project-flow CLI dispatch: connect, list, disconnect, specs, dependencies.
//!
//! Split out of `lib.rs` so the cli-app crate stays under the workspace
//! 500-line line-budget.
//!
//! Each dispatch function constructs a typed [`ActorContext`] from the
//! CLI-supplied `account_id` argument rather than embedding authority
//! inside the project command body. The app-service layer evaluates
//! policy using this context.

use std::io::Write;

use anyhow::{Context, Result};
use tanren_app_services::{ActorContext, AppServiceError, Handlers, Store};
use tanren_contract::{
    ConnectProjectRequest, DependencyView, DisconnectProjectRequest, ProjectDependenciesResponse,
    ProjectFailureBody, ProjectSpecsResponse, SpecView,
};
use tanren_identity_policy::{AccountId, OrgId, ProjectId};

pub(crate) async fn connect_project(
    handlers: &Handlers,
    store: &Store,
    _database_url: &str,
    account_id: AccountId,
    org_id: OrgId,
    name: String,
    repository_url: String,
) -> Result<()> {
    let actor = ActorContext::from_account_id(account_id);
    let request = ConnectProjectRequest {
        account_id: None,
        org_id,
        name,
        repository_url,
    };
    let response = handlers
        .connect_project(store, &actor, request)
        .await
        .map_err(project_error)?;
    write_json_line(&response)
}

pub(crate) async fn list_projects(
    handlers: &Handlers,
    store: &Store,
    _database_url: &str,
    account_id: AccountId,
) -> Result<()> {
    let actor = ActorContext::from_account_id(account_id);
    let response = handlers
        .list_projects(store, &actor)
        .await
        .map_err(project_error)?;
    write_json_line(&response)
}

pub(crate) async fn disconnect_project(
    handlers: &Handlers,
    store: &Store,
    _database_url: &str,
    project_id: ProjectId,
    account_id: AccountId,
) -> Result<()> {
    let actor = ActorContext::from_account_id(account_id);
    let request = DisconnectProjectRequest {
        project_id,
        account_id: None,
    };
    let response = handlers
        .disconnect_project(store, &actor, request)
        .await
        .map_err(project_error)?;
    write_json_line(&response)
}

pub(crate) async fn project_specs(
    handlers: &Handlers,
    store: &Store,
    account_id: AccountId,
    project_id: ProjectId,
) -> Result<()> {
    let actor = ActorContext::from_account_id(account_id);
    let specs = handlers
        .project_specs(store, &actor, project_id)
        .await
        .map_err(project_error)?;
    let response = ProjectSpecsResponse {
        specs: specs
            .into_iter()
            .map(|s| SpecView {
                id: s.id,
                project_id: s.project_id,
                title: s.title,
                created_at: s.created_at,
            })
            .collect(),
    };
    write_json_line(&response)
}

pub(crate) async fn project_dependencies(
    handlers: &Handlers,
    store: &Store,
    account_id: AccountId,
    project_id: ProjectId,
) -> Result<()> {
    let actor = ActorContext::from_account_id(account_id);
    let deps = handlers
        .project_dependencies(store, &actor, project_id)
        .await
        .map_err(project_error)?;
    let response = ProjectDependenciesResponse {
        dependencies: deps
            .into_iter()
            .map(|d| DependencyView {
                source_project_id: d.source_project_id,
                source_spec_id: d.source_spec_id,
                target_project_id: d.target_project_id,
                resolved: d.resolved,
                detected_at: d.detected_at,
            })
            .collect(),
    };
    write_json_line(&response)
}

fn project_error(err: AppServiceError) -> anyhow::Error {
    match err {
        AppServiceError::Project(reason) => {
            let body = ProjectFailureBody::from_reason(reason);
            let msg =
                serde_json::to_string(&body).unwrap_or_else(|_| format!("error: {}", body.code));
            anyhow::anyhow!("{msg}")
        }
        AppServiceError::InvalidInput(message) => {
            anyhow::anyhow!(r#"{{"code":"validation_failed","summary":"{message}"}}"#)
        }
        AppServiceError::Store(store_err) => {
            anyhow::anyhow!(r#"{{"code":"internal_error","summary":"{store_err}"}}"#)
        }
        _ => anyhow::anyhow!(
            r#"{{"code":"internal_error","summary":"unknown app-service failure"}}"#
        ),
    }
}

fn write_json_line<T: serde::Serialize>(value: &T) -> Result<()> {
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    let json = serde_json::to_string(value).context("serialize JSON output")?;
    writeln!(handle, "{json}").context("write JSON output")
}
