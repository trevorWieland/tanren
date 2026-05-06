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
//!
//! Input validation uses the bounded typed inputs from
//! `tanren_contract` ([`ProjectName`], [`ProviderHost`],
//! [`RepositoryUrl`]) to enforce length limits and reject URLs with
//! credentials, query strings, or fragments *before* any provider or
//! store call.

use chrono::{DateTime, Utc};
use tanren_contract::project::RepositoryView;
use tanren_contract::{
    ActiveProjectView, ConnectProjectRequest, CreateProjectRequest, ProjectContentCounts,
    ProjectFailureReason, ProjectName, ProjectView, ProviderHost, RepositoryUrl,
    normalize_repository_identity,
};
use tanren_identity_policy::{AccountId, ProjectId, RepositoryId};
use tanren_provider_integrations::{
    HostId, ProviderAction, ProviderConnectionContext, SourceControlProvider,
};
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
    let name = ProjectName::parse(&request.name)
        .map_err(|_| AppServiceError::Project(ProjectFailureReason::ValidationFailed))?;
    let url = RepositoryUrl::parse(&request.repository_url)
        .map_err(|_| AppServiceError::Project(ProjectFailureReason::ValidationFailed))?;
    let now = clock.now();

    let identity = normalize_repository_identity(url.as_str());
    if store
        .find_project_by_repository_identity(&identity)
        .await?
        .is_some()
    {
        return Err(AppServiceError::Project(
            ProjectFailureReason::DuplicateRepository,
        ));
    }

    let Some(host_str) = url.host() else {
        return Err(AppServiceError::Project(
            ProjectFailureReason::ValidationFailed,
        ));
    };
    let host_id = HostId::new(host_str.to_owned());

    let context = ProviderConnectionContext {
        actor: account_id,
        host: host_id,
        action: ProviderAction::CheckRepoAccess {
            url: url.as_str().to_owned(),
        },
    };
    if let Err(err) = scm.check_repo_access(&context).await {
        let reason = map_provider_error(&err);
        emit_connect_rejected(store, reason, &url.redacted(), now).await?;
        return Err(AppServiceError::Project(reason));
    }

    let project_id = ProjectId::fresh();
    let repository_id = RepositoryId::fresh();

    let output = match store
        .register_project_atomic(
            NewProject {
                id: project_id,
                name: name.as_str().to_owned(),
                repository_id,
                owner_account_id: account_id,
                owner_org_id: request.org,
                repository_identity: identity,
                repository_url: url.as_str().to_owned(),
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
    let name = ProjectName::parse(&request.name)
        .map_err(|_| AppServiceError::Project(ProjectFailureReason::ValidationFailed))?;
    let host = ProviderHost::parse(&request.provider_host)
        .map_err(|_| AppServiceError::Project(ProjectFailureReason::ValidationFailed))?;
    let now = clock.now();

    let host_id = HostId::new(host.as_str().to_owned());

    let context = ProviderConnectionContext {
        actor: account_id,
        host: host_id,
        action: ProviderAction::CreateRepository {
            name: name.as_str().to_owned(),
        },
    };
    let repo_info = match scm.create_repository(&context).await {
        Ok(info) => info,
        Err(err) => {
            let reason = map_provider_error(&err);
            emit_create_rejected(store, reason, host.as_str(), now).await?;
            return Err(AppServiceError::Project(reason));
        }
    };

    let url = RepositoryUrl::parse(&repo_info.url)
        .map_err(|_| AppServiceError::Project(ProjectFailureReason::ValidationFailed))?;
    let identity = normalize_repository_identity(url.as_str());
    let project_id = ProjectId::fresh();
    let repository_id = RepositoryId::fresh();

    let output = match store
        .register_project_atomic(
            NewProject {
                id: project_id,
                name: name.as_str().to_owned(),
                repository_id,
                owner_account_id: account_id,
                owner_org_id: request.org,
                repository_identity: identity,
                repository_url: url.as_str().to_owned(),
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

fn map_provider_error(err: &tanren_provider_integrations::ProviderError) -> ProjectFailureReason {
    match err {
        tanren_provider_integrations::ProviderError::HostAccess(_) => {
            ProjectFailureReason::AccessDenied
        }
        tanren_provider_integrations::ProviderError::Call(_) => {
            ProjectFailureReason::ProviderFailure
        }
        tanren_provider_integrations::ProviderError::NotConfigured => {
            ProjectFailureReason::ProviderNotConfigured
        }
        _ => ProjectFailureReason::ProviderFailure,
    }
}

async fn emit_connect_rejected<S>(
    store: &S,
    reason: ProjectFailureReason,
    redacted_url: &str,
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
                    repository_url: redacted_url.to_owned(),
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
