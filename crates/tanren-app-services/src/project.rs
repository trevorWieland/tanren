//! Project-setup handlers shared across every interface surface.
//!
//! This module owns the two R-0019 command flows:
//! - connect an existing repository as a project;
//! - create a new repository at a designated host, then register it as a project.

use tanren_contract::{
    ActiveProjectRequest, ActiveProjectView, ConnectProjectRepositoryRequest,
    ConnectProjectRepositoryResponse, CreateProjectRequest, CreateProjectResponse,
    ListVisibleProjectsRequest, PROJECT_LIST_DEFAULT_PAGE_SIZE, PROJECT_LIST_MAX_PAGE_SIZE,
    ProjectCollectionFreshnessView, ProjectCollectionView, ProjectCountsView, ProjectFailureReason,
    ProjectListCursor, ProjectPaginationView, ProjectRepositoryView, ProjectSelectionView,
    ProjectView,
};
use tanren_identity_policy::{AccountId, DesignatedHost, ProjectId, ProviderFamily, RepositoryRef};
use tanren_provider_integrations::{SourceControlError, SourceControlProvider};
use tanren_store::{
    NewProject, NewProjectRepository, ProjectListCursor as StoreProjectListCursor,
    ProjectSetupRecord, ProjectStore, ProjectStoreError,
};

use crate::{AppServiceError, Clock};

/// Command envelope for connecting an existing repository.
#[derive(Debug, Clone)]
pub struct ConnectExistingRepositoryCommand {
    /// Authenticated actor attempting the command.
    pub actor_account_id: AccountId,
    /// Command payload.
    pub request: ConnectProjectRepositoryRequest,
}

/// Command envelope for creating a new project repository.
#[derive(Debug, Clone)]
pub struct CreateNewProjectCommand {
    /// Authenticated actor attempting the command.
    pub actor_account_id: AccountId,
    /// Command payload.
    pub request: CreateProjectRequest,
}

/// Query envelope for listing visible projects.
#[derive(Debug, Clone)]
pub struct ListVisibleProjectsQuery {
    /// Authenticated actor issuing the query.
    pub actor_account_id: AccountId,
    /// Query payload.
    pub request: ListVisibleProjectsRequest,
}

/// Query envelope for reading active-project metadata.
#[derive(Debug, Clone)]
pub struct ActiveProjectQuery {
    /// Authenticated actor issuing the query.
    pub actor_account_id: AccountId,
    /// Query payload.
    pub request: ActiveProjectRequest,
}

pub(crate) async fn connect_existing_repository<S, P>(
    store: &S,
    provider: &P,
    clock: &Clock,
    command: ConnectExistingRepositoryCommand,
) -> Result<ConnectProjectRepositoryResponse, AppServiceError>
where
    S: ProjectStore + ?Sized,
    P: SourceControlProvider + ?Sized,
{
    validate_actor_scope(
        store,
        command.actor_account_id,
        command.request.owning_account_id,
    )
    .await?;

    provider
        .ensure_provider_reachable()
        .await
        .map_err(map_provider_error)?;
    let has_repo_access = provider
        .can_access_repository(command.actor_account_id, &command.request.repository)
        .await
        .map_err(map_provider_error)?;
    if !has_repo_access {
        return Err(AppServiceError::Project(ProjectFailureReason::NoAccess));
    }

    let provider_family = provider.family();
    let designated_host = DesignatedHost::parse(provider_family.as_str())
        .map_err(|_| AppServiceError::Project(ProjectFailureReason::ValidationFailed))?;

    let setup = register_project_repository(
        store,
        command.request.owning_account_id,
        provider_family,
        designated_host,
        command.request.repository,
        command.request.select_as_active,
        clock,
    )
    .await?;

    Ok(ConnectProjectRepositoryResponse {
        project: project_view(&setup),
    })
}

pub(crate) async fn create_new_project<S, P>(
    store: &S,
    provider: &P,
    clock: &Clock,
    command: CreateNewProjectCommand,
) -> Result<CreateProjectResponse, AppServiceError>
where
    S: ProjectStore + ?Sized,
    P: SourceControlProvider + ?Sized,
{
    let designated_host = &command.request.designated_host;
    let provider_family = provider.family();

    validate_actor_scope(
        store,
        command.actor_account_id,
        command.request.owning_account_id,
    )
    .await?;
    ensure_repository_not_registered(
        store,
        command.request.owning_account_id,
        &provider_family,
        &command.request.repository,
    )
    .await?;

    provider
        .ensure_provider_reachable()
        .await
        .map_err(map_provider_error)?;
    provider
        .ensure_host_reachable(designated_host)
        .await
        .map_err(map_provider_error)?;

    let can_create = provider
        .can_create_repository_at_host(command.actor_account_id, designated_host)
        .await
        .map_err(map_provider_error)?;
    if !can_create {
        return Err(AppServiceError::Project(ProjectFailureReason::NoAccess));
    }

    let created_repository = provider
        .create_repository(
            command.actor_account_id,
            designated_host,
            &command.request.repository,
        )
        .await
        .map_err(map_provider_error)?;

    let setup = register_project_repository(
        store,
        command.request.owning_account_id,
        provider_family,
        designated_host.clone(),
        created_repository,
        command.request.select_as_active,
        clock,
    )
    .await?;
    // If a concurrent command registers the same account+provider+repository
    // between the preflight lookup and this insert, the store returns
    // `DuplicateRepository` and this call propagates the same conflict taxonomy.
    // Callers reconcile by re-listing visible projects and using the existing
    // registration as the canonical binding.

    Ok(CreateProjectResponse {
        project: project_view(&setup),
    })
}

pub(crate) async fn list_visible_projects<S>(
    store: &S,
    query: ListVisibleProjectsQuery,
) -> Result<ProjectCollectionView, AppServiceError>
where
    S: ProjectStore + ?Sized,
{
    validate_actor_scope(
        store,
        query.actor_account_id,
        query.request.owning_account_id,
    )
    .await?;
    let page_size = bounded_page_size(query.request.page.page_size);
    let cursor = query
        .request
        .page
        .cursor
        .as_ref()
        .map(to_store_project_cursor);
    let page = store
        .list_projects_for_account(query.request.owning_account_id, page_size, cursor.as_ref())
        .await?;
    let projects = page.projects.iter().map(project_view).collect();
    Ok(ProjectCollectionView {
        owning_account_id: query.request.owning_account_id,
        projects,
        pagination: ProjectPaginationView {
            page_size: page.page_size,
            default_page_size: PROJECT_LIST_DEFAULT_PAGE_SIZE,
            max_page_size: PROJECT_LIST_MAX_PAGE_SIZE,
            has_more: page.has_more,
            next_cursor: page.next_cursor.as_ref().map(to_contract_project_cursor),
        },
        freshness: ProjectCollectionFreshnessView { as_of: page.as_of },
    })
}

pub(crate) async fn active_project<S>(
    store: &S,
    query: ActiveProjectQuery,
) -> Result<ActiveProjectView, AppServiceError>
where
    S: ProjectStore + ?Sized,
{
    validate_actor_scope(
        store,
        query.actor_account_id,
        query.request.owning_account_id,
    )
    .await?;
    let active_project = store
        .active_project_for_account(query.request.owning_account_id)
        .await?
        .as_ref()
        .map(project_view);
    Ok(ActiveProjectView {
        owning_account_id: query.request.owning_account_id,
        active_project,
    })
}

async fn validate_actor_scope<S>(
    store: &S,
    actor_account_id: AccountId,
    owning_account_id: AccountId,
) -> Result<(), AppServiceError>
where
    S: ProjectStore + ?Sized,
{
    if actor_account_id != owning_account_id {
        return Err(AppServiceError::Project(ProjectFailureReason::NoAccess));
    }

    if !store.account_exists(owning_account_id).await? {
        return Err(AppServiceError::Project(ProjectFailureReason::NoAccess));
    }

    Ok(())
}

async fn ensure_repository_not_registered<S>(
    store: &S,
    owning_account_id: AccountId,
    provider_family: &ProviderFamily,
    repository: &RepositoryRef,
) -> Result<(), AppServiceError>
where
    S: ProjectStore + ?Sized,
{
    if store
        .find_project_repository(owning_account_id, provider_family, repository)
        .await?
        .is_some()
    {
        return Err(AppServiceError::Project(
            ProjectFailureReason::DuplicateRepository,
        ));
    }

    Ok(())
}

async fn register_project_repository<S>(
    store: &S,
    owning_account_id: AccountId,
    provider_family: ProviderFamily,
    designated_host: DesignatedHost,
    repository: RepositoryRef,
    select_as_active: bool,
    clock: &Clock,
) -> Result<ProjectSetupRecord, AppServiceError>
where
    S: ProjectStore + ?Sized,
{
    let now = clock.now();
    let project_id = ProjectId::fresh();
    let setup = store
        .create_project_setup(
            NewProject {
                id: project_id,
                owning_account_id,
                created_at: now,
                active_selected_at: None,
            },
            NewProjectRepository {
                project_id,
                owning_account_id,
                repository_ref: repository,
                provider_family,
                designated_host,
                created_at: now,
            },
            select_as_active,
        )
        .await
        .map_err(map_project_store_error)?;

    Ok(setup)
}

fn bounded_page_size(requested: u16) -> u16 {
    if requested == 0 {
        PROJECT_LIST_DEFAULT_PAGE_SIZE
    } else {
        requested.min(PROJECT_LIST_MAX_PAGE_SIZE)
    }
}

fn to_store_project_cursor(cursor: &ProjectListCursor) -> StoreProjectListCursor {
    StoreProjectListCursor {
        active_selected_at: cursor.active_selected_at,
        created_at: cursor.created_at,
        project_id: cursor.project_id,
    }
}

fn to_contract_project_cursor(cursor: &StoreProjectListCursor) -> ProjectListCursor {
    ProjectListCursor {
        active_selected_at: cursor.active_selected_at,
        created_at: cursor.created_at,
        project_id: cursor.project_id,
    }
}

fn map_provider_error(err: SourceControlError) -> AppServiceError {
    match err {
        SourceControlError::ProviderUnavailable => {
            AppServiceError::Project(ProjectFailureReason::ProviderUnavailable)
        }
        SourceControlError::ProviderUnreachable
        | SourceControlError::HostUnreachable
        | SourceControlError::OperationFailed => {
            AppServiceError::Project(ProjectFailureReason::ProviderFailure)
        }
        _ => AppServiceError::Project(ProjectFailureReason::ProviderFailure),
    }
}

fn map_project_store_error(err: ProjectStoreError) -> AppServiceError {
    match err {
        ProjectStoreError::DuplicateRepository => {
            AppServiceError::Project(ProjectFailureReason::DuplicateRepository)
        }
        ProjectStoreError::Store(err) => AppServiceError::Store(err),
    }
}

fn project_view(setup: &ProjectSetupRecord) -> ProjectView {
    ProjectView {
        id: setup.project.id,
        owning_account_id: setup.project.owning_account_id,
        repository: ProjectRepositoryView {
            provider_family: setup.repository.provider_family.clone(),
            repository: setup.repository.repository_ref.clone(),
        },
        selection: ProjectSelectionView {
            is_active: setup.is_active,
            selected_at: setup.project.active_selected_at,
        },
        counts: ProjectCountsView {
            specs: setup.spec_count,
            milestones: setup.milestone_count,
            initiatives: setup.initiative_count,
        },
        created_at: setup.project.created_at,
    }
}
