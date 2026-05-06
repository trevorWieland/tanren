//! Project-scoped HTTP route handlers.
//!
//! Split out of `routes.rs` so the api-app crate stays under the workspace
//! 500-line line-budget.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use tanren_contract::{AttentionSpecView, ProjectView, SwitchProjectResponse};
use tanren_identity_policy::{LoopId, MilestoneId, ProjectId, SpecId};
use tower_sessions::Session;

use crate::AppState;
use crate::cookies::read_session_account_id;
use crate::errors::{AccountFailureBody, map_app_error, unauthenticated_error};

/// Response body for `GET /projects/active/views`.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ScopedViewsResponse {
    pub project_id: ProjectId,
    pub specs: Vec<SpecId>,
    pub loops: Vec<LoopId>,
    pub milestones: Vec<MilestoneId>,
    #[schema(value_type = Object)]
    pub view_state: Option<serde_json::Value>,
}

/// List every project in the caller's account with attention indicators.
#[utoipa::path(
    get,
    path = "/projects",
    responses(
        (status = 200, body = Vec<ProjectView>, description = "Project list"),
        (status = 401, body = AccountFailureBody, description = "unauthenticated"),
    ),
    tag = "projects",
)]
pub(crate) async fn list_projects_route(
    State(state): State<AppState>,
    session: Session,
) -> Response {
    let Ok(account_id) = read_session_account_id(&session).await else {
        return unauthenticated_error();
    };
    match state
        .handlers
        .list_projects(state.store.as_ref(), account_id)
        .await
    {
        Ok(projects) => (StatusCode::OK, Json(projects)).into_response(),
        Err(err) => map_app_error(err),
    }
}

/// Fetch a single flagged attention spec within a project.
#[utoipa::path(
    get,
    path = "/projects/{project_id}/specs/{spec_id}/attention",
    params(
        ("project_id" = ProjectId, Path, description = "Project id"),
        ("spec_id" = SpecId, Path, description = "Spec id"),
    ),
    responses(
        (status = 200, body = AttentionSpecView, description = "Attention spec detail"),
        (status = 401, body = AccountFailureBody, description = "unauthenticated"),
        (status = 403, body = AccountFailureBody, description = "unauthorized_project_access"),
        (status = 404, body = AccountFailureBody, description = "unknown_spec"),
    ),
    tag = "projects",
)]
pub(crate) async fn attention_spec_route(
    State(state): State<AppState>,
    session: Session,
    Path((project_id, spec_id)): Path<(ProjectId, SpecId)>,
) -> Response {
    let Ok(account_id) = read_session_account_id(&session).await else {
        return unauthenticated_error();
    };
    match state
        .handlers
        .attention_spec(state.store.as_ref(), account_id, project_id, spec_id)
        .await
    {
        Ok(view) => (StatusCode::OK, Json(view)).into_response(),
        Err(err) => map_app_error(err),
    }
}

/// Switch the active project for the session account.
#[utoipa::path(
    post,
    path = "/projects/{project_id}/switch",
    params(
        ("project_id" = ProjectId, Path, description = "Project to activate"),
    ),
    responses(
        (status = 200, body = SwitchProjectResponse, description = "Project switched"),
        (status = 401, body = AccountFailureBody, description = "unauthenticated"),
        (status = 403, body = AccountFailureBody, description = "unauthorized_project_access"),
    ),
    tag = "projects",
)]
pub(crate) async fn switch_project_route(
    State(state): State<AppState>,
    session: Session,
    Path(project_id): Path<ProjectId>,
) -> Response {
    let Ok(account_id) = read_session_account_id(&session).await else {
        return unauthenticated_error();
    };
    match state
        .handlers
        .switch_active_project(state.store.as_ref(), account_id, project_id)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
}

/// Read specs, loops, milestones, and view state for the active project.
#[utoipa::path(
    get,
    path = "/projects/active/views",
    responses(
        (status = 200, body = ScopedViewsResponse, description = "Scoped views"),
        (status = 401, body = AccountFailureBody, description = "unauthenticated"),
    ),
    tag = "projects",
)]
pub(crate) async fn active_project_views_route(
    State(state): State<AppState>,
    session: Session,
) -> Response {
    let Ok(account_id) = read_session_account_id(&session).await else {
        return unauthenticated_error();
    };
    match state
        .handlers
        .project_scoped_views(state.store.as_ref(), account_id)
        .await
    {
        Ok(views) => (
            StatusCode::OK,
            Json(ScopedViewsResponse {
                project_id: views.project_id,
                specs: views.specs,
                loops: views.loops,
                milestones: views.milestones,
                view_state: views.view_state,
            }),
        )
            .into_response(),
        Err(err) => map_app_error(err),
    }
}
