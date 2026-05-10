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
    ProjectListCursor, ProjectListSelectionFilter, ProjectListSortOrder, ProjectPaginationView,
    ProjectRepositoryView, ProjectSelectionView, ProjectView,
};
use tanren_identity_policy::{AccountId, ProjectId};
use tanren_provider_integrations::{
    SourceControlError, SourceControlProvider, SourceControlRateLimitStatus,
    SourceControlRemoteIdentity,
};
use tanren_store::{
    NewProject, NewProjectRepository, ProjectListCursor as StoreProjectListCursor,
    ProjectSetupRecord, ProjectStore, ProjectStoreError,
};

use crate::project_command_reservations::{complete_or_fail_reservation, reserve_project_command};
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

    let provider_family = provider.family();
    let reservation = reserve_project_command(
        store,
        command.request.owning_account_id,
        &provider_family,
        &command.request.repository,
        clock.now(),
    )
    .await?;

    let result =
        connect_existing_repository_with_reservation(store, provider, clock, command).await;
    complete_or_fail_reservation(store, &reservation, clock, result).await
}

async fn connect_existing_repository_with_reservation<S, P>(
    store: &S,
    provider: &P,
    clock: &Clock,
    command: ConnectExistingRepositoryCommand,
) -> Result<ConnectProjectRepositoryResponse, AppServiceError>
where
    S: ProjectStore + ?Sized,
    P: SourceControlProvider + ?Sized,
{
    let preflight = provider
        .preflight_connect_repository(command.actor_account_id, &command.request.repository)
        .await
        .map_err(map_provider_error)?;
    if matches!(
        preflight.rate_limit,
        SourceControlRateLimitStatus::RateLimited
    ) {
        return Err(AppServiceError::Project(ProjectFailureReason::RateLimited));
    }
    if !preflight.repository_access {
        return Err(AppServiceError::Project(ProjectFailureReason::NoAccess));
    }

    let setup = register_project_repository(
        store,
        command.request.owning_account_id,
        preflight.remote_identity,
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
    let provider_family = provider.family();

    validate_actor_scope(
        store,
        command.actor_account_id,
        command.request.owning_account_id,
    )
    .await?;
    let reservation = reserve_project_command(
        store,
        command.request.owning_account_id,
        &provider_family,
        &command.request.repository,
        clock.now(),
    )
    .await?;

    let result = create_new_project_with_reservation(store, provider, clock, command).await;
    complete_or_fail_reservation(store, &reservation, clock, result).await
}

async fn create_new_project_with_reservation<S, P>(
    store: &S,
    provider: &P,
    clock: &Clock,
    command: CreateNewProjectCommand,
) -> Result<CreateProjectResponse, AppServiceError>
where
    S: ProjectStore + ?Sized,
    P: SourceControlProvider + ?Sized,
{
    let preflight = provider
        .preflight_create_repository(
            command.actor_account_id,
            &command.request.designated_host,
            &command.request.repository,
        )
        .await
        .map_err(map_provider_error)?;
    if matches!(
        preflight.rate_limit,
        SourceControlRateLimitStatus::RateLimited
    ) {
        return Err(AppServiceError::Project(ProjectFailureReason::RateLimited));
    }
    if !preflight.host_create_access {
        return Err(AppServiceError::Project(ProjectFailureReason::NoAccess));
    }
    let designated_host = preflight.remote_identity.designated_host.clone();

    let created_repository = provider
        .create_repository(
            command.actor_account_id,
            &designated_host,
            &preflight.remote_identity.repository,
        )
        .await
        .map_err(map_provider_error)?;

    let setup_result = register_project_repository(
        store,
        command.request.owning_account_id,
        SourceControlRemoteIdentity {
            repository: created_repository.clone(),
            ..preflight.remote_identity
        },
        command.request.select_as_active,
        clock,
    )
    .await;
    let setup = match setup_result {
        Ok(setup) => setup,
        Err(err @ AppServiceError::Project(ProjectFailureReason::DuplicateRepository)) => {
            return Err(err);
        }
        Err(err) => {
            if let Err(cleanup_err) = provider
                .delete_repository(
                    command.actor_account_id,
                    &designated_host,
                    &created_repository,
                )
                .await
                .map_err(map_provider_error)
            {
                let _ = cleanup_err;
            }
            return Err(err);
        }
    };

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
    validate_project_list_filter(query.request.page.filter.selection)?;
    validate_project_list_sort(query.request.page.sort.order)?;
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

fn validate_project_list_filter(filter: ProjectListSelectionFilter) -> Result<(), AppServiceError> {
    match filter {
        ProjectListSelectionFilter::All => Ok(()),
        _ => Err(AppServiceError::Project(
            ProjectFailureReason::ValidationFailed,
        )),
    }
}

fn validate_project_list_sort(sort: ProjectListSortOrder) -> Result<(), AppServiceError> {
    match sort {
        ProjectListSortOrder::ActiveSelectedThenCreatedDesc => Ok(()),
        _ => Err(AppServiceError::Project(
            ProjectFailureReason::ValidationFailed,
        )),
    }
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

async fn register_project_repository<S>(
    store: &S,
    owning_account_id: AccountId,
    remote_identity: SourceControlRemoteIdentity,
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
                repository_ref: remote_identity.repository,
                provider_family: remote_identity.provider_family,
                designated_host: remote_identity.designated_host,
                provider_remote_id: remote_identity.provider_remote_id,
                provider_remote_url: remote_identity.provider_remote_url,
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
        SourceControlError::Unauthorized => {
            AppServiceError::Project(ProjectFailureReason::NoAccess)
        }
        SourceControlError::RateLimited => {
            AppServiceError::Project(ProjectFailureReason::RateLimited)
        }
        SourceControlError::Conflict => {
            AppServiceError::Project(ProjectFailureReason::DuplicateRepository)
        }
        SourceControlError::ProviderUnreachable
        | SourceControlError::HostUnreachable
        | SourceControlError::OperationFailed => {
            AppServiceError::Project(ProjectFailureReason::ProviderFailure)
        }
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
            source_control_host: setup.repository.designated_host.clone(),
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
