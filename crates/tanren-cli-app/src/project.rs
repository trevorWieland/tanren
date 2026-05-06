//! Project-flow CLI dispatch: connect, list, disconnect, specs, dependencies.
//!
//! Split out of `lib.rs` so the cli-app crate stays under the workspace
//! 500-line line-budget.

use std::io::Write;

use anyhow::{Context, Result};
use tanren_app_services::{AppServiceError, Handlers, Store};
use tanren_contract::{ConnectProjectRequest, DisconnectProjectRequest};
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
    let request = ConnectProjectRequest {
        account_id,
        org_id,
        name,
        repository_url,
    };
    let response = handlers
        .connect_project(store, request)
        .await
        .map_err(project_error)?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(
        handle,
        "project_id={id} name={name} org_id={org} repository_url={repo} connected_at={at}",
        id = response.project.id,
        name = response.project.name,
        org = response.project.org_id,
        repo = response.project.repository_url,
        at = response.project.connected_at.to_rfc3339(),
    )
    .context("write connect-project result")?;
    Ok(())
}

pub(crate) async fn list_projects(
    handlers: &Handlers,
    store: &Store,
    _database_url: &str,
    account_id: AccountId,
) -> Result<()> {
    let response = handlers
        .list_projects(store, account_id)
        .await
        .map_err(project_error)?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    if response.projects.is_empty() {
        writeln!(handle, "projects: (none)").context("write list-projects result")?;
    } else {
        for p in &response.projects {
            writeln!(
                handle,
                "project_id={id} name={name} org_id={org} repository_url={repo} connected_at={at}",
                id = p.id,
                name = p.name,
                org = p.org_id,
                repo = p.repository_url,
                at = p.connected_at.to_rfc3339(),
            )
            .context("write list-projects result")?;
        }
    }
    Ok(())
}

pub(crate) async fn disconnect_project(
    handlers: &Handlers,
    store: &Store,
    _database_url: &str,
    project_id: ProjectId,
    account_id: AccountId,
) -> Result<()> {
    let request = DisconnectProjectRequest {
        project_id,
        account_id,
    };
    let response = handlers
        .disconnect_project(store, request)
        .await
        .map_err(project_error)?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(
        handle,
        "disconnected project_id={id}",
        id = response.project_id,
    )
    .context("write disconnect-project result")?;
    for dep in &response.unresolved_inbound_dependencies {
        writeln!(
            handle,
            "unresolved source_project_id={src} source_spec_id={spec} target_project_id={tgt}",
            src = dep.source_project_id,
            spec = dep.source_spec_id,
            tgt = dep.unresolved_target_project_id,
        )
        .context("write unresolved dependency signal")?;
    }
    Ok(())
}

pub(crate) async fn project_specs(
    handlers: &Handlers,
    store: &Store,
    project_id: ProjectId,
) -> Result<()> {
    let specs = handlers
        .project_specs(store, project_id)
        .await
        .map_err(project_error)?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    if specs.is_empty() {
        writeln!(handle, "specs: (none)").context("write project-specs result")?;
    } else {
        for s in &specs {
            writeln!(
                handle,
                "spec_id={id} project_id={pid} title={title} created_at={at}",
                id = s.id,
                pid = s.project_id,
                title = s.title,
                at = s.created_at.to_rfc3339(),
            )
            .context("write project-specs result")?;
        }
    }
    Ok(())
}

pub(crate) async fn project_dependencies(
    handlers: &Handlers,
    store: &Store,
    project_id: ProjectId,
) -> Result<()> {
    let deps = handlers
        .project_dependencies(store, project_id)
        .await
        .map_err(project_error)?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    if deps.is_empty() {
        writeln!(handle, "dependencies: (none)").context("write project-dependencies result")?;
    } else {
        for d in &deps {
            let status = if d.resolved { "resolved" } else { "unresolved" };
            writeln!(
                handle,
                "source_project_id={src} source_spec_id={spec} target_project_id={tgt} status={status}",
                src = d.source_project_id,
                spec = d.source_spec_id,
                tgt = d.target_project_id,
                status = status,
            )
            .context("write project-dependencies result")?;
        }
    }
    Ok(())
}

fn project_error(err: AppServiceError) -> anyhow::Error {
    match err {
        AppServiceError::Project(reason) => {
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
