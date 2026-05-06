//! Project-flow handlers: connect, list, disconnect, specs, dependencies.
//!
//! Handlers consume `&dyn ProjectStore` (and `&dyn AccountStore` for event
//! emission) so every interface binary (api/cli/mcp/tui) shares identical
//! behaviour. The underlying repository path supplied in connect requests is
//! stored as metadata only — the handler never writes to or deletes the
//! repository directory.
//!
//! Every handler receives a typed [`ActorContext`] and evaluates
//! [`tanren_policy`] before proceeding. Interface layers construct the actor
//! context from their authenticated session (API/MCP) or local identity
//! (CLI/TUI) — never from a raw `account_id` in the request body.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tanren_contract::{
    ConnectProjectRequest, ConnectProjectResponse, DisconnectProjectRequest,
    DisconnectProjectResponse, ListProjectsResponse, ProjectDependencyResponse,
    ProjectFailureReason, ProjectView, ReconnectProjectResponse,
};
use tanren_identity_policy::{ProjectId, SpecId};
use tanren_policy::{ActorContext, Decision, ProjectAction, evaluate_project_policy};
use tanren_provider_integrations::ProviderRegistry;
use tanren_store::{
    AccountStore, DependencyLinkStatus, DisconnectProjectError, NewProject, ProjectStatus,
    ProjectStore, ReconnectProjectError,
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

pub(crate) async fn connect_project<S, P>(
    store: &S,
    clock: &Clock,
    actor: &ActorContext,
    providers: &P,
    request: ConnectProjectRequest,
) -> Result<ConnectProjectResponse, AppServiceError>
where
    S: AccountStore + ProjectStore + ?Sized,
    P: ProviderRegistry + ?Sized,
{
    let name = request.name.trim().to_owned();
    let resource_id = request.resource_id.trim().to_owned();
    let now = clock.now();

    if name.is_empty() || resource_id.is_empty() {
        return Err(AppServiceError::Project(
            ProjectFailureReason::ValidationFailed,
        ));
    }

    let org_ids = store.account_org_memberships(actor.account_id()).await?;
    let resolved_actor = ActorContext::new(actor.account_id(), org_ids);
    let decision = evaluate_project_policy(
        &resolved_actor,
        ProjectAction::Connect,
        Some(request.org_id),
        false,
    );
    if !matches!(decision, Decision::Allow) {
        return Err(AppServiceError::Project(ProjectFailureReason::Unauthorized));
    }

    let provider =
        providers
            .get(request.provider_connection_id)
            .await
            .ok_or(AppServiceError::Project(
                ProjectFailureReason::ProviderConnectionNotFound,
            ))?;

    let caps = provider.capabilities();
    if !caps.can_read || !caps.can_assess_merge_permissions {
        return Err(AppServiceError::Project(
            ProjectFailureReason::InsufficientProviderCapabilities,
        ));
    }

    let resource = provider
        .resolve_resource(&resource_id)
        .await
        .map_err(|_| AppServiceError::Project(ProjectFailureReason::RepositoryUnavailable))?;

    let merge_perms = provider
        .merge_permissions(&resource)
        .await
        .map_err(|_| AppServiceError::Project(ProjectFailureReason::RepositoryUnavailable))?;
    if !merge_perms.can_push_to_default {
        return Err(AppServiceError::Project(
            ProjectFailureReason::RepositoryUnavailable,
        ));
    }

    if let Some(existing) = store
        .find_project_by_org_and_resource(
            request.org_id,
            request.provider_connection_id,
            &resource_id,
        )
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
        .insert_project(NewProject {
            id: project_id,
            org_id: request.org_id,
            name: name.clone(),
            provider_connection_id: request.provider_connection_id,
            resource_id,
            display_ref: resource.display_ref.clone(),
            connected_at: now,
        })
        .await?;

    store
        .append_event(
            project_envelope(
                ProjectEventKind::ProjectConnected,
                &ProjectConnected {
                    project_id: record.id,
                    org_id: record.org_id,
                    name,
                    display_ref: record.display_ref.clone(),
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
    actor: &ActorContext,
) -> Result<ListProjectsResponse, AppServiceError>
where
    S: ProjectStore + ?Sized,
{
    let _ = evaluate_project_policy(actor, ProjectAction::List, None, true);
    let records = store
        .list_connected_projects_for_account(actor.account_id())
        .await?;
    Ok(ListProjectsResponse {
        projects: records.iter().map(project_view).collect(),
    })
}

pub(crate) async fn disconnect_project<S>(
    store: &S,
    clock: &Clock,
    actor: &ActorContext,
    request: DisconnectProjectRequest,
) -> Result<DisconnectProjectResponse, AppServiceError>
where
    S: AccountStore + ProjectStore + ?Sized,
{
    let now = clock.now();

    let can_see = store
        .account_can_see_project(actor.account_id(), request.project_id)
        .await?;
    let decision = evaluate_project_policy(actor, ProjectAction::Disconnect, None, can_see);
    if !matches!(decision, Decision::Allow) {
        return Err(AppServiceError::Project(ProjectFailureReason::Unauthorized));
    }

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
        .list_connected_projects_for_account(actor.account_id())
        .await?;

    Ok(DisconnectProjectResponse {
        project_id: record.id,
        account_projects: remaining.iter().map(project_view).collect(),
        unresolved_inbound_dependencies: unresolved,
    })
}

pub(crate) async fn project_specs<S>(
    store: &S,
    actor: &ActorContext,
    project_id: ProjectId,
) -> Result<Vec<ProjectSpecView>, AppServiceError>
where
    S: ProjectStore + ?Sized,
{
    let can_see = store
        .account_can_see_project(actor.account_id(), project_id)
        .await?;
    let decision = evaluate_project_policy(actor, ProjectAction::Specs, None, can_see);
    if !matches!(decision, Decision::Allow) {
        return Err(AppServiceError::Project(ProjectFailureReason::Unauthorized));
    }

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
    actor: &ActorContext,
    project_id: ProjectId,
) -> Result<Vec<ProjectDependencyView>, AppServiceError>
where
    S: ProjectStore + ?Sized,
{
    let can_see = store
        .account_can_see_project(actor.account_id(), project_id)
        .await?;
    let decision = evaluate_project_policy(actor, ProjectAction::Dependencies, None, can_see);
    if !matches!(decision, Decision::Allow) {
        return Err(AppServiceError::Project(ProjectFailureReason::Unauthorized));
    }

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

pub(crate) async fn reconnect_project<S>(
    store: &S,
    actor: &ActorContext,
    project_id: ProjectId,
) -> Result<ReconnectProjectResponse, AppServiceError>
where
    S: ProjectStore + ?Sized,
{
    let can_see = store
        .account_can_see_project(actor.account_id(), project_id)
        .await?;
    let decision = evaluate_project_policy(actor, ProjectAction::Reconnect, None, can_see);
    if !matches!(decision, Decision::Allow) {
        return Err(AppServiceError::Project(ProjectFailureReason::Unauthorized));
    }

    let reconnected = store
        .reconnect_project(project_id)
        .await
        .map_err(|e| match e {
            ReconnectProjectError::NotFound => {
                AppServiceError::Project(ProjectFailureReason::ProjectNotFound)
            }
            ReconnectProjectError::Store(e) => AppServiceError::Store(e),
        })?;
    Ok(ReconnectProjectResponse {
        project: project_view(&reconnected.project),
    })
}

fn project_view(record: &tanren_store::ProjectRecord) -> ProjectView {
    ProjectView {
        id: record.id,
        name: record.name.clone(),
        org_id: record.org_id,
        display_ref: record.display_ref.clone(),
        connected_at: record.connected_at,
        disconnected_at: match record.status {
            ProjectStatus::Disconnected(at) => Some(at),
            ProjectStatus::Connected => None,
        },
    }
}
