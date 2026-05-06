//! Project-flow handlers: connect, list, disconnect, specs, dependencies.
//!
//! Handlers consume `&dyn ProjectStore` (and `&dyn AccountStore` for event
//! emission) so every interface binary (api/cli/mcp/tui) shares identical
//! behaviour. The underlying repository path supplied in connect requests is
//! stored as metadata only — the handler never writes to or deletes the
//! repository directory.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tanren_contract::{
    ConnectProjectRequest, ConnectProjectResponse, DisconnectProjectRequest,
    DisconnectProjectResponse, ListProjectsResponse, ProjectDependencyResponse,
    ProjectFailureReason, ProjectView,
};
use tanren_identity_policy::{AccountId, ProjectId, SpecId};
use tanren_store::{
    AccountStore, DependencyLinkStatus, DisconnectProjectError, ProjectStatus, ProjectStore,
};

use crate::events::{
    CrossProjectDependencyUnresolved, ProjectConnected, ProjectDisconnectRejected,
    ProjectDisconnected, ProjectEventKind, project_envelope,
};
use crate::{AppServiceError, Clock};

/// External-facing view of a spec attached to a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSpecView {
    pub id: SpecId,
    pub project_id: ProjectId,
    pub title: String,
    pub created_at: DateTime<Utc>,
}

/// External-facing view of a cross-project dependency link annotated with
/// resolution status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDependencyView {
    pub source_project_id: ProjectId,
    pub source_spec_id: SpecId,
    pub target_project_id: ProjectId,
    pub resolved: bool,
    pub detected_at: DateTime<Utc>,
}

pub(crate) async fn connect_project<S>(
    store: &S,
    clock: &Clock,
    request: ConnectProjectRequest,
) -> Result<ConnectProjectResponse, AppServiceError>
where
    S: AccountStore + ProjectStore + ?Sized,
{
    let name = request.name.trim().to_owned();
    let repository_url = request.repository_url.trim().to_owned();
    let now = clock.now();

    if name.is_empty() || repository_url.is_empty() {
        return Err(AppServiceError::Project(
            ProjectFailureReason::ValidationFailed,
        ));
    }

    if let Some(existing) = store
        .find_project_by_org_and_repo(request.org_id, &repository_url)
        .await?
    {
        if matches!(existing.status, ProjectStatus::Connected) {
            return Err(AppServiceError::Project(
                ProjectFailureReason::RepositoryUnavailable,
            ));
        }
    }

    let project_id = ProjectId::fresh();
    let record = store
        .insert_project(
            project_id,
            request.org_id,
            name.clone(),
            repository_url.clone(),
            now,
        )
        .await?;

    store
        .append_event(
            project_envelope(
                ProjectEventKind::ProjectConnected,
                &ProjectConnected {
                    project_id: record.id,
                    org_id: record.org_id,
                    name,
                    repository_url,
                    at: now,
                },
            ),
            now,
        )
        .await?;

    Ok(ConnectProjectResponse {
        project: project_view(&record),
    })
}

pub(crate) async fn list_projects<S>(
    store: &S,
    account_id: AccountId,
) -> Result<ListProjectsResponse, AppServiceError>
where
    S: ProjectStore + ?Sized,
{
    let records = store
        .list_connected_projects_for_account(account_id)
        .await?;
    Ok(ListProjectsResponse {
        projects: records.iter().map(project_view).collect(),
    })
}

pub(crate) async fn disconnect_project<S>(
    store: &S,
    clock: &Clock,
    request: DisconnectProjectRequest,
) -> Result<DisconnectProjectResponse, AppServiceError>
where
    S: AccountStore + ProjectStore + ?Sized,
{
    let now = clock.now();

    if store.has_active_loop_fixtures(request.project_id).await? {
        store
            .append_event(
                project_envelope(
                    ProjectEventKind::ProjectDisconnectRejected,
                    &ProjectDisconnectRejected {
                        project_id: request.project_id,
                        reason: ProjectFailureReason::ActiveLoopExists,
                        at: now,
                    },
                ),
                now,
            )
            .await?;
        return Err(AppServiceError::Project(
            ProjectFailureReason::ActiveLoopExists,
        ));
    }

    let record = match store.disconnect_project(request.project_id, now).await {
        Ok(r) => r,
        Err(DisconnectProjectError::NotFound) => {
            return Err(AppServiceError::Project(
                ProjectFailureReason::ProjectNotFound,
            ));
        }
        Err(DisconnectProjectError::Store(e)) => return Err(AppServiceError::Store(e)),
    };

    let inbound = store.read_inbound_dependencies(request.project_id).await?;
    let mut unresolved = Vec::with_capacity(inbound.len());
    for link in &inbound {
        store
            .append_event(
                project_envelope(
                    ProjectEventKind::CrossProjectDependencyUnresolved,
                    &CrossProjectDependencyUnresolved {
                        source_project_id: link.dependency.source_project_id,
                        source_spec_id: link.dependency.source_spec_id,
                        unresolved_target_project_id: link.dependency.target_project_id,
                        detected_at: now,
                    },
                ),
                now,
            )
            .await?;
        unresolved.push(ProjectDependencyResponse {
            source_project_id: link.dependency.source_project_id,
            source_spec_id: link.dependency.source_spec_id,
            unresolved_target_project_id: link.dependency.target_project_id,
            detected_at: link.dependency.detected_at,
        });
    }

    store
        .append_event(
            project_envelope(
                ProjectEventKind::ProjectDisconnected,
                &ProjectDisconnected {
                    project_id: record.id,
                    at: now,
                },
            ),
            now,
        )
        .await?;

    let remaining = store
        .list_connected_projects_for_account(request.account_id)
        .await?;

    Ok(DisconnectProjectResponse {
        project_id: record.id,
        account_projects: remaining.iter().map(project_view).collect(),
        unresolved_inbound_dependencies: unresolved,
    })
}

pub(crate) async fn project_specs<S>(
    store: &S,
    project_id: ProjectId,
) -> Result<Vec<ProjectSpecView>, AppServiceError>
where
    S: ProjectStore + ?Sized,
{
    let records = store.read_project_specs(project_id).await?;
    Ok(records
        .into_iter()
        .map(|r| ProjectSpecView {
            id: r.id,
            project_id: r.project_id,
            title: r.title,
            created_at: r.created_at,
        })
        .collect())
}

pub(crate) async fn project_dependencies<S>(
    store: &S,
    project_id: ProjectId,
) -> Result<Vec<ProjectDependencyView>, AppServiceError>
where
    S: ProjectStore + ?Sized,
{
    let links = store.read_project_dependencies(project_id).await?;
    Ok(links
        .into_iter()
        .map(|link| ProjectDependencyView {
            source_project_id: link.dependency.source_project_id,
            source_spec_id: link.dependency.source_spec_id,
            target_project_id: link.dependency.target_project_id,
            resolved: link.status == DependencyLinkStatus::Resolved,
            detected_at: link.dependency.detected_at,
        })
        .collect())
}

fn project_view(record: &tanren_store::ProjectRecord) -> ProjectView {
    ProjectView {
        id: record.id,
        name: record.name.clone(),
        org_id: record.org_id,
        repository_url: record.repository_url.clone(),
        connected_at: record.connected_at,
        disconnected_at: match record.status {
            ProjectStatus::Disconnected(at) => Some(at),
            ProjectStatus::Connected => None,
        },
    }
}
