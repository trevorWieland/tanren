//! TUI project-flow: form factories, outcome adapters, and dispatch helpers.
//!
//! Split out of `app.rs` so the tui-app crate stays under the workspace
//! 500-line line-budget.
//!
//! Each dispatch function constructs a typed [`ActorContext`] from the
//! form-supplied `account_id` field rather than embedding authority inside
//! the project command body.

use std::sync::Arc;

use tanren_app_services::{ActorContext, AppServiceError, Handlers, Store};
use tanren_contract::{ProjectFailureReason, ProjectView};
use tanren_identity_policy::{AccountId, ProjectId};
use tokio::runtime::Runtime;
use uuid::Uuid;

use crate::{FormField, FormState, OutcomeView};

pub(crate) fn connect_project_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Account ID",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Org ID",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Project name",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Repository URL",
            secret: false,
            value: String::new(),
        },
    ]
}

pub(crate) fn list_projects_fields() -> Vec<FormField> {
    vec![FormField {
        label: "Account ID",
        secret: false,
        value: String::new(),
    }]
}

pub(crate) fn disconnect_project_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Project ID",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Account ID",
            secret: false,
            value: String::new(),
        },
    ]
}

pub(crate) fn project_dependencies_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Project ID",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Account ID",
            secret: false,
            value: String::new(),
        },
    ]
}

pub(crate) fn parse_connect_project(
    state: &FormState,
) -> Result<(AccountId, tanren_identity_policy::OrgId, String, String), String> {
    let account_id = parse_uuid_field(state.value(0), "Account ID")?;
    let org_id = parse_uuid_field(state.value(1), "Org ID")?;
    let name = state.value(2).trim().to_owned();
    let repository_url = state.value(3).trim().to_owned();
    if name.is_empty() {
        return Err("validation_failed: project name is required".to_owned());
    }
    if repository_url.is_empty() {
        return Err("validation_failed: repository URL is required".to_owned());
    }
    Ok((
        AccountId::new(account_id),
        tanren_identity_policy::OrgId::new(org_id),
        name,
        repository_url,
    ))
}

pub(crate) fn parse_account_id(state: &FormState) -> Result<AccountId, String> {
    let uuid = parse_uuid_field(state.value(0), "Account ID")?;
    Ok(AccountId::new(uuid))
}

pub(crate) fn parse_project_account_ids(
    state: &FormState,
) -> Result<(ProjectId, AccountId), String> {
    let project_id = parse_uuid_field(state.value(0), "Project ID")?;
    let account_id = parse_uuid_field(state.value(1), "Account ID")?;
    Ok((ProjectId::new(project_id), AccountId::new(account_id)))
}

fn parse_uuid_field(raw: &str, label: &str) -> Result<Uuid, String> {
    raw.trim()
        .parse::<Uuid>()
        .map_err(|_| format!("validation_failed: {label} must be a valid UUID"))
}

pub(crate) fn connect_project_outcome(project: &ProjectView) -> OutcomeView {
    OutcomeView {
        title: "Project connected",
        lines: vec![
            format!("project_id: {}", project.id),
            format!("name: {}", project.name),
            format!("org_id: {}", project.org_id),
            format!("repository_url: {}", project.repository_url),
            format!("connected_at: {}", project.connected_at),
        ],
    }
}

pub(crate) fn list_projects_outcome(projects: &[ProjectView]) -> OutcomeView {
    let mut lines = Vec::with_capacity(projects.len() + 1);
    if projects.is_empty() {
        lines.push("(no connected projects)".to_owned());
    } else {
        for p in projects {
            lines.push(format!(
                "{} | {} | {} | {}",
                p.id, p.name, p.org_id, p.repository_url
            ));
        }
    }
    OutcomeView {
        title: "Projects",
        lines,
    }
}

pub(crate) fn disconnect_project_outcome(
    project_id: ProjectId,
    unresolved: &[tanren_contract::ProjectDependencyResponse],
) -> OutcomeView {
    let mut lines = vec![format!("disconnected project_id: {project_id}")];
    if unresolved.is_empty() {
        lines.push("no unresolved inbound dependencies".to_owned());
    } else {
        lines.push(format!(
            "unresolved inbound dependencies: {}",
            unresolved.len()
        ));
        for dep in unresolved {
            lines.push(format!(
                "  {} -> {} (from spec {})",
                dep.source_project_id, dep.unresolved_target_project_id, dep.source_spec_id,
            ));
        }
    }
    OutcomeView {
        title: "Project disconnected",
        lines,
    }
}

pub(crate) fn project_dependencies_outcome(
    deps: &[tanren_app_services::project::ProjectDependencyView],
) -> OutcomeView {
    let mut lines = Vec::new();
    if deps.is_empty() {
        lines.push("(no dependencies)".to_owned());
    } else {
        for d in deps {
            let status = if d.resolved { "resolved" } else { "unresolved" };
            lines.push(format!(
                "{} -> {} (from spec {}) [{status}]",
                d.source_project_id, d.target_project_id, d.source_spec_id,
            ));
        }
    }
    OutcomeView {
        title: "Project dependencies",
        lines,
    }
}

pub(crate) fn render_project_error(err: AppServiceError) -> String {
    match err {
        AppServiceError::Project(reason) => format_project_failure(reason),
        AppServiceError::InvalidInput(message) => format!("validation_failed: {message}"),
        AppServiceError::Store(err) => format!("internal_error: {err}"),
        _ => "internal_error: unknown app-service failure".to_owned(),
    }
}

fn format_project_failure(reason: ProjectFailureReason) -> String {
    format!("{}: {}", reason.code(), reason.summary())
}

pub(crate) enum ProjectActionResult {
    Outcome(OutcomeView),
    Error(String),
}

pub(crate) fn dispatch_connect_project(
    runtime: &Runtime,
    handlers: &Handlers,
    store: &Arc<Store>,
    state: &FormState,
) -> ProjectActionResult {
    let (account_id, org_id, name, repository_url) = match parse_connect_project(state) {
        Ok(vals) => vals,
        Err(message) => return ProjectActionResult::Error(message),
    };
    let actor = ActorContext::from_account_id(account_id);
    let request = tanren_contract::ConnectProjectRequest {
        account_id: None,
        org_id,
        name,
        repository_url,
    };
    match runtime.block_on(handlers.connect_project(store.as_ref(), &actor, request)) {
        Ok(response) => ProjectActionResult::Outcome(connect_project_outcome(&response.project)),
        Err(reason) => ProjectActionResult::Error(render_project_error(reason)),
    }
}

pub(crate) fn dispatch_list_projects(
    runtime: &Runtime,
    handlers: &Handlers,
    store: &Arc<Store>,
    state: &FormState,
) -> ProjectActionResult {
    let account_id = match parse_account_id(state) {
        Ok(id) => id,
        Err(message) => return ProjectActionResult::Error(message),
    };
    let actor = ActorContext::from_account_id(account_id);
    match runtime.block_on(handlers.list_projects(store.as_ref(), &actor)) {
        Ok(response) => ProjectActionResult::Outcome(list_projects_outcome(&response.projects)),
        Err(reason) => ProjectActionResult::Error(render_project_error(reason)),
    }
}

pub(crate) fn dispatch_disconnect_project(
    runtime: &Runtime,
    handlers: &Handlers,
    store: &Arc<Store>,
    state: &FormState,
) -> ProjectActionResult {
    let (project_id, account_id) = match parse_project_account_ids(state) {
        Ok(vals) => vals,
        Err(message) => return ProjectActionResult::Error(message),
    };
    let actor = ActorContext::from_account_id(account_id);
    let request = tanren_contract::DisconnectProjectRequest {
        project_id,
        account_id: None,
    };
    match runtime.block_on(handlers.disconnect_project(store.as_ref(), &actor, request)) {
        Ok(response) => ProjectActionResult::Outcome(disconnect_project_outcome(
            response.project_id,
            &response.unresolved_inbound_dependencies,
        )),
        Err(reason) => ProjectActionResult::Error(render_project_error(reason)),
    }
}

pub(crate) fn dispatch_project_dependencies(
    runtime: &Runtime,
    handlers: &Handlers,
    store: &Arc<Store>,
    state: &FormState,
) -> ProjectActionResult {
    let (project_id, account_id) = match parse_project_account_ids(state) {
        Ok(vals) => vals,
        Err(message) => return ProjectActionResult::Error(message),
    };
    let actor = ActorContext::from_account_id(account_id);
    match runtime.block_on(handlers.project_dependencies(store.as_ref(), &actor, project_id)) {
        Ok(deps) => ProjectActionResult::Outcome(project_dependencies_outcome(&deps)),
        Err(reason) => ProjectActionResult::Error(render_project_error(reason)),
    }
}
