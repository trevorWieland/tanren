//! Project-flow handlers: list, attention drill-down, active-project
//! switching, and scoped views.
//!
//! Handlers are mechanism-neutral at the contract surface. Each handler
//! consumes `&dyn ProjectStore` (the port defined in `tanren_store::traits`);
//! the SeaORM-backed `Store` is the adapter injected by interface binaries.
//!
//! Attention semantics are fixture-based at M-0003 time: the `needs_attention`
//! boolean on each spec row is seeded by test fixtures and aggregated here
//! into project-level indicators. M-0007 owns real spec lifecycle semantics.

use serde::{Deserialize, Serialize};
use tanren_contract::{
    AttentionSpecView, ProjectFailureReason, ProjectScopedViews, ProjectStateSummary, ProjectView,
    SwitchProjectResponse,
};
use tanren_identity_policy::{AccountId, LoopId, MilestoneId, ProjectId, SpecId};
use tanren_store::ProjectStore;

use crate::{AppServiceError, Clock};

pub(crate) async fn list_projects<S>(
    store: &S,
    account_id: AccountId,
) -> Result<Vec<ProjectView>, AppServiceError>
where
    S: ProjectStore + ?Sized,
{
    let projects = store.list_projects(account_id).await?;
    let mut views = Vec::with_capacity(projects.len());
    for project in &projects {
        let attention_specs = store.find_attention_specs(project.id).await?;
        let needs_attention = !attention_specs.is_empty();
        let spec_views = attention_specs
            .into_iter()
            .map(attention_spec_view)
            .collect();
        views.push(ProjectView {
            id: project.id,
            name: project.name.clone(),
            state: parse_project_state(&project.state),
            needs_attention,
            attention_specs: spec_views,
            created_at: project.created_at,
        });
    }
    Ok(views)
}

pub(crate) async fn attention_spec<S>(
    store: &S,
    account_id: AccountId,
    project_id: ProjectId,
    spec_id: SpecId,
) -> Result<AttentionSpecView, AppServiceError>
where
    S: ProjectStore + ?Sized,
{
    let projects = store.list_projects(account_id).await?;
    if !projects.iter().any(|p| p.id == project_id) {
        return Err(AppServiceError::Project(
            ProjectFailureReason::UnauthorizedProjectAccess,
        ));
    }
    let specs = store.find_attention_specs(project_id).await?;
    specs
        .into_iter()
        .find(|s| s.id == spec_id)
        .map(attention_spec_view)
        .ok_or_else(|| AppServiceError::Project(ProjectFailureReason::UnknownSpec))
}

pub(crate) async fn switch_active_project<S>(
    store: &S,
    clock: &Clock,
    account_id: AccountId,
    project_id: ProjectId,
) -> Result<SwitchProjectResponse, AppServiceError>
where
    S: ProjectStore + ?Sized,
{
    let now = clock.now();
    store
        .write_active_project(account_id, project_id, now)
        .await?;

    let project = store
        .list_projects(account_id)
        .await?
        .into_iter()
        .find(|p| p.id == project_id)
        .ok_or_else(|| AppServiceError::Project(ProjectFailureReason::UnknownProject))?;

    let attention_specs = store.find_attention_specs(project.id).await?;
    let needs_attention = !attention_specs.is_empty();
    let spec_views = attention_specs
        .into_iter()
        .map(attention_spec_view)
        .collect();
    let project_view = ProjectView {
        id: project.id,
        name: project.name,
        state: parse_project_state(&project.state),
        needs_attention,
        attention_specs: spec_views,
        created_at: project.created_at,
    };

    let scoped_views = store.read_scoped_views(project_id).await?;
    let scoped = ProjectScopedViews {
        project_id,
        specs: scoped_views.spec_ids,
        loops: scoped_views.loop_ids,
        milestones: scoped_views.milestone_ids,
    };

    Ok(SwitchProjectResponse {
        project: project_view,
        scoped,
    })
}

/// Response returned by project-scoped-views queries. Extends the contract's
/// [`ProjectScopedViews`] with the per-account persisted view state
/// so callers can restore the exact UI position for the active project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectScopedViewsResponse {
    /// The project these views are scoped to.
    pub project_id: ProjectId,
    /// Specs belonging to the active project.
    pub specs: Vec<SpecId>,
    /// Loops belonging to the active project.
    pub loops: Vec<LoopId>,
    /// Milestones belonging to the active project.
    pub milestones: Vec<MilestoneId>,
    /// Persisted per-account view state for the active project.
    /// `None` until the account has previously interacted with this
    /// project's views.
    pub view_state: Option<serde_json::Value>,
}

pub(crate) async fn project_scoped_views<S>(
    store: &S,
    account_id: AccountId,
) -> Result<ProjectScopedViewsResponse, AppServiceError>
where
    S: ProjectStore + ?Sized,
{
    let active = store
        .read_active_project(account_id)
        .await?
        .ok_or_else(|| AppServiceError::InvalidInput("no active project".to_owned()))?;

    let scoped = store.read_scoped_views(active.project_id).await?;
    let view_state = store.read_view_state(account_id, active.project_id).await?;

    Ok(ProjectScopedViewsResponse {
        project_id: active.project_id,
        specs: scoped.spec_ids,
        loops: scoped.loop_ids,
        milestones: scoped.milestone_ids,
        view_state,
    })
}

fn attention_spec_view(spec: tanren_store::SpecRecord) -> AttentionSpecView {
    AttentionSpecView {
        id: spec.id,
        name: spec.name,
        reason: spec.attention_reason.unwrap_or_default(),
    }
}

fn parse_project_state(raw: &str) -> ProjectStateSummary {
    match raw {
        "paused" => ProjectStateSummary::Paused,
        "completed" => ProjectStateSummary::Completed,
        "archived" => ProjectStateSummary::Archived,
        _ => ProjectStateSummary::Active,
    }
}
