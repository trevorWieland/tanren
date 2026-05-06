//! Project-flow handlers: connect existing repository (B-0025) and
//! create new project + repository (B-0026).
//!
//! Both pathways register exactly one project backed by exactly one
//! repository, set the new project as the caller's active project, and
//! start with empty content counts (no specs / milestones /
//! initiatives are scaffolded). Prior repository history is never
//! imported as Tanren activity events.
//!
//! Handlers consume `&S` where `S: ProjectStore + AccountStore` (the
//! combined port defined in `tanren_store::traits`) and `&P` where
//! `P: SourceControlProvider` (the SCM provider trait defined in
//! `tanren_provider_integrations`). The SeaORM-backed `Store` and a
//! concrete provider adapter are injected by interface binaries.

use chrono::{DateTime, Utc};
use tanren_contract::project::RepositoryView;
use tanren_contract::{
    ActiveProjectView, ConnectProjectRequest, CreateProjectRequest, ProjectContentCounts,
    ProjectFailureReason, ProjectView,
};
use tanren_identity_policy::{AccountId, ProjectId, RepositoryId};
use tanren_provider_integrations::{HostId, SourceControlProvider};
use tanren_store::{AccountStore, NewProject, ProjectRecord, ProjectStore, RegisterProjectError};

use crate::events::{
    ProjectConnectRejected, ProjectConnected, ProjectCreateRejected, ProjectCreated,
    ProjectEventKinds, project_envelope,
};
use crate::{AppServiceError, Clock};

pub(crate) async fn connect_project<S, P>(
    store: &S,
    scm: &P,
    clock: &Clock,
    account_id: AccountId,
    request: ConnectProjectRequest,
) -> Result<ProjectView, AppServiceError>
where
    S: ProjectStore + AccountStore + ?Sized,
    P: SourceControlProvider + ?Sized,
{
    let name = request.name.trim().to_owned();
    let url = request.repository_url.trim().to_owned();
    let now = clock.now();

    if name.is_empty() || url.is_empty() {
        return Err(AppServiceError::Project(
            ProjectFailureReason::ValidationFailed,
        ));
    }

    let identity = normalize_repository_identity(&url);
    if store
        .find_project_by_repository_identity(&identity)
        .await?
        .is_some()
    {
        return Err(AppServiceError::Project(
            ProjectFailureReason::DuplicateRepository,
        ));
    }

    let Some(host) = extract_host(&url) else {
        emit_connect_rejected(store, ProjectFailureReason::ValidationFailed, &url, now).await?;
        return Err(AppServiceError::Project(
            ProjectFailureReason::ValidationFailed,
        ));
    };
    let host_id = HostId::new(host);

    if let Err(err) = scm.check_repo_access(&host_id, &url).await {
        let reason = map_provider_error(&err);
        emit_connect_rejected(store, reason, &url, now).await?;
        return Err(AppServiceError::Project(reason));
    }

    let project_id = ProjectId::fresh();
    let repository_id = RepositoryId::fresh();

    let output = match store
        .register_project_atomic(
            NewProject {
                id: project_id,
                name,
                repository_id,
                owner_account_id: account_id,
                owner_org_id: request.org,
                repository_identity: identity,
                repository_url: url.clone(),
                created_at: now,
            },
            now,
        )
        .await
    {
        Ok(o) => o,
        Err(RegisterProjectError::DuplicateRepository) => {
            return Err(AppServiceError::Project(
                ProjectFailureReason::DuplicateRepository,
            ));
        }
        Err(RegisterProjectError::Store(err)) => {
            return Err(AppServiceError::Store(err));
        }
    };

    store
        .append_event(
            project_envelope(
                ProjectEventKinds::PROJECT_CONNECTED,
                &ProjectConnected {
                    project_id: output.project.id,
                    repository_id: output.project.repository_id,
                    owner: account_id,
                    at: now,
                },
            ),
            now,
        )
        .await?;

    Ok(project_view(&output.project))
}

pub(crate) async fn create_project<S, P>(
    store: &S,
    scm: &P,
    clock: &Clock,
    account_id: AccountId,
    request: CreateProjectRequest,
) -> Result<ProjectView, AppServiceError>
where
    S: ProjectStore + AccountStore + ?Sized,
    P: SourceControlProvider + ?Sized,
{
    let name = request.name.trim().to_owned();
    let host_str = request.provider_host.trim().to_owned();
    let now = clock.now();

    if name.is_empty() || host_str.is_empty() {
        return Err(AppServiceError::Project(
            ProjectFailureReason::ValidationFailed,
        ));
    }

    let host_id = HostId::new(host_str.clone());

    let repo_info = match scm.create_repository(&host_id, &name).await {
        Ok(info) => info,
        Err(err) => {
            let reason = map_provider_error(&err);
            emit_create_rejected(store, reason, &host_str, now).await?;
            return Err(AppServiceError::Project(reason));
        }
    };

    let identity = normalize_repository_identity(&repo_info.url);
    let project_id = ProjectId::fresh();
    let repository_id = RepositoryId::fresh();

    let output = match store
        .register_project_atomic(
            NewProject {
                id: project_id,
                name,
                repository_id,
                owner_account_id: account_id,
                owner_org_id: request.org,
                repository_identity: identity,
                repository_url: repo_info.url.clone(),
                created_at: now,
            },
            now,
        )
        .await
    {
        Ok(o) => o,
        Err(RegisterProjectError::DuplicateRepository) => {
            return Err(AppServiceError::Project(
                ProjectFailureReason::DuplicateRepository,
            ));
        }
        Err(RegisterProjectError::Store(err)) => {
            return Err(AppServiceError::Store(err));
        }
    };

    store
        .append_event(
            project_envelope(
                ProjectEventKinds::PROJECT_CREATED,
                &ProjectCreated {
                    project_id: output.project.id,
                    repository_id: output.project.repository_id,
                    owner: account_id,
                    at: now,
                },
            ),
            now,
        )
        .await?;

    Ok(project_view(&output.project))
}

pub(crate) async fn active_project<S>(
    store: &S,
    account_id: AccountId,
) -> Result<Option<ActiveProjectView>, AppServiceError>
where
    S: ProjectStore + ?Sized,
{
    let Some(active) = store.get_active_project(account_id).await? else {
        return Ok(None);
    };
    let project = store.find_project_by_id(active.project_id).await?;
    match project {
        Some(p) => Ok(Some(ActiveProjectView {
            project: project_view(&p),
            activated_at: active.selected_at,
        })),
        None => Ok(None),
    }
}

fn project_view(record: &ProjectRecord) -> ProjectView {
    ProjectView {
        id: record.id,
        name: record.name.clone(),
        repository: RepositoryView {
            id: record.repository_id,
            url: record.repository_url.clone(),
        },
        owner: record.owner_account_id,
        org: record.owner_org_id,
        created_at: record.created_at,
        content_counts: ProjectContentCounts::empty(),
    }
}

fn normalize_repository_identity(url: &str) -> String {
    let stripped = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .or_else(|| url.strip_prefix("ssh://"))
        .or_else(|| url.strip_prefix("git@"))
        .unwrap_or(url);
    let replaced = stripped.replace(':', "/");
    let trimmed = replaced.strip_suffix(".git").unwrap_or(replaced.as_str());
    let trimmed = trimmed.strip_suffix('/').unwrap_or(trimmed);
    trimmed.to_lowercase()
}

fn extract_host(url: &str) -> Option<String> {
    let stripped = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .or_else(|| url.strip_prefix("ssh://"))
        .or_else(|| url.strip_prefix("git@"))?;
    let host = stripped.split(['/', ':']).next()?;
    if host.is_empty() {
        return None;
    }
    Some(host.to_lowercase())
}

fn map_provider_error(err: &tanren_provider_integrations::ProviderError) -> ProjectFailureReason {
    match err {
        tanren_provider_integrations::ProviderError::HostAccess(_) => {
            ProjectFailureReason::AccessDenied
        }
        tanren_provider_integrations::ProviderError::Call(_) => {
            ProjectFailureReason::ProviderFailure
        }
        _ => ProjectFailureReason::ProviderFailure,
    }
}

async fn emit_connect_rejected<S>(
    store: &S,
    reason: ProjectFailureReason,
    repository_url: &str,
    now: DateTime<Utc>,
) -> Result<(), AppServiceError>
where
    S: AccountStore + ?Sized,
{
    store
        .append_event(
            project_envelope(
                ProjectEventKinds::PROJECT_CONNECT_REJECTED,
                &ProjectConnectRejected {
                    reason,
                    repository_url: repository_url.to_owned(),
                    at: now,
                },
            ),
            now,
        )
        .await?;
    Ok(())
}

async fn emit_create_rejected<S>(
    store: &S,
    reason: ProjectFailureReason,
    provider_host: &str,
    now: DateTime<Utc>,
) -> Result<(), AppServiceError>
where
    S: AccountStore + ?Sized,
{
    store
        .append_event(
            project_envelope(
                ProjectEventKinds::PROJECT_CREATE_REJECTED,
                &ProjectCreateRejected {
                    reason,
                    provider_host: provider_host.to_owned(),
                    at: now,
                },
            ),
            now,
        )
        .await?;
    Ok(())
}
