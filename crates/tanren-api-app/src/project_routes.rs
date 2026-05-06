//! Project-flow API route handlers.
//!
//! Split out of `routes.rs` so both modules stay under the workspace
//! 500-line line-budget. The connect endpoint also handles reconnecting
//! a previously disconnected project — the handler decides based on
//! existing state.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use tanren_contract::{
    ConnectProjectRequest, ConnectProjectResponse, DependencyView, DisconnectProjectBody,
    DisconnectProjectRequest, DisconnectProjectResponse, ListProjectsParams, ListProjectsResponse,
    ProjectDependenciesResponse, ProjectSpecsResponse, SpecView,
};
use tanren_identity_policy::ProjectId;

use crate::AppState;
use crate::errors::{ValidatedJson, map_app_error};
use tanren_contract::ProjectFailureBody;

/// Connect (or reconnect) a repository as a Tanren project.
#[utoipa::path(
    post,
    path = "/projects",
    request_body = ConnectProjectRequest,
    responses(
        (status = 201, body = ConnectProjectResponse, description = "Project connected (or reconnected)"),
        (status = 400, body = ProjectFailureBody, description = "validation_failed"),
        (status = 409, body = ProjectFailureBody, description = "repository_unavailable or active_loop_exists"),
    ),
    tag = "projects",
)]
pub(crate) async fn connect_project_route(
    State(state): State<AppState>,
    ValidatedJson(request): ValidatedJson<ConnectProjectRequest>,
) -> Response {
    match state
        .handlers
        .connect_project(state.store.as_ref(), request)
        .await
    {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
}

/// List projects accessible to the caller.
#[utoipa::path(
    get,
    path = "/projects",
    params(
        ("account_id" = String, Query, description = "Account whose projects to list"),
    ),
    responses(
        (status = 200, body = ListProjectsResponse, description = "Project list"),
        (status = 400, body = ProjectFailureBody, description = "validation_failed"),
    ),
    tag = "projects",
)]
pub(crate) async fn list_projects_route(
    State(state): State<AppState>,
    Query(query): Query<ListProjectsParams>,
) -> Response {
    match state
        .handlers
        .list_projects(state.store.as_ref(), query.account_id)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
}

/// Disconnect a project from Tanren. The underlying repository is not
/// modified. Returns unresolved inbound dependency signals for any
/// cross-project links that pointed into the disconnected project.
#[utoipa::path(
    post,
    path = "/projects/{id}/disconnect",
    request_body = DisconnectProjectBody,
    params(
        ("id" = String, Path, description = "Project id to disconnect"),
    ),
    responses(
        (status = 200, body = DisconnectProjectResponse, description = "Project disconnected, unresolved dependencies signalled"),
        (status = 400, body = ProjectFailureBody, description = "validation_failed"),
        (status = 404, body = ProjectFailureBody, description = "project_not_found"),
        (status = 409, body = ProjectFailureBody, description = "active_loop_exists"),
    ),
    tag = "projects",
)]
pub(crate) async fn disconnect_project_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
    ValidatedJson(body): ValidatedJson<DisconnectProjectBody>,
) -> Response {
    let project_id = match id.parse() {
        Ok(uuid) => ProjectId::new(uuid),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ProjectFailureBody {
                    code: "validation_failed".to_owned(),
                    summary: "project_id must be a valid UUID".to_owned(),
                }),
            )
                .into_response();
        }
    };
    let request = DisconnectProjectRequest {
        project_id,
        account_id: body.account_id,
    };
    match state
        .handlers
        .disconnect_project(state.store.as_ref(), request)
        .await
    {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
}

/// List specs attached to a project.
#[utoipa::path(
    get,
    path = "/projects/{id}/specs",
    params(
        ("id" = String, Path, description = "Project whose specs to list"),
    ),
    responses(
        (status = 200, body = ProjectSpecsResponse, description = "Project specs"),
        (status = 404, body = ProjectFailureBody, description = "project_not_found"),
    ),
    tag = "projects",
)]
pub(crate) async fn project_specs_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let project_id = match id.parse() {
        Ok(uuid) => ProjectId::new(uuid),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ProjectFailureBody {
                    code: "validation_failed".to_owned(),
                    summary: "project_id must be a valid UUID".to_owned(),
                }),
            )
                .into_response();
        }
    };
    match state
        .handlers
        .project_specs(state.store.as_ref(), project_id)
        .await
    {
        Ok(specs) => {
            let views = specs
                .into_iter()
                .map(|s| SpecView {
                    id: s.id,
                    project_id: s.project_id,
                    title: s.title,
                    created_at: s.created_at,
                })
                .collect();
            (StatusCode::OK, Json(ProjectSpecsResponse { specs: views })).into_response()
        }
        Err(err) => map_app_error(err),
    }
}

/// List cross-project dependency links for a project.
#[utoipa::path(
    get,
    path = "/projects/{id}/dependencies",
    params(
        ("id" = String, Path, description = "Project whose dependencies to list"),
    ),
    responses(
        (status = 200, body = ProjectDependenciesResponse, description = "Project dependencies"),
        (status = 404, body = ProjectFailureBody, description = "project_not_found"),
    ),
    tag = "projects",
)]
pub(crate) async fn project_dependencies_route(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let project_id = match id.parse() {
        Ok(uuid) => ProjectId::new(uuid),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ProjectFailureBody {
                    code: "validation_failed".to_owned(),
                    summary: "project_id must be a valid UUID".to_owned(),
                }),
            )
                .into_response();
        }
    };
    match state
        .handlers
        .project_dependencies(state.store.as_ref(), project_id)
        .await
    {
        Ok(deps) => {
            let views = deps
                .into_iter()
                .map(|d| DependencyView {
                    source_project_id: d.source_project_id,
                    source_spec_id: d.source_spec_id,
                    target_project_id: d.target_project_id,
                    resolved: d.resolved,
                    detected_at: d.detected_at,
                })
                .collect();
            (
                StatusCode::OK,
                Json(ProjectDependenciesResponse {
                    dependencies: views,
                }),
            )
                .into_response()
        }
        Err(err) => map_app_error(err),
    }
}
