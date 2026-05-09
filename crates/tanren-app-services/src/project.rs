//! Project-setup handlers shared across every interface surface.
//!
//! This module owns the two R-0019 command flows:
//! - connect an existing repository as a project;
//! - create a new repository at a designated host, then register it as a project.

use tanren_contract::{
    ConnectProjectRepositoryRequest, ConnectProjectRepositoryResponse, CreateProjectRequest,
    CreateProjectResponse, ProjectCountsView, ProjectFailureReason, ProjectRepositoryView,
    ProjectSelectionView, ProjectView,
};
use tanren_identity_policy::{AccountId, ProjectId, RepositoryRef};
use tanren_provider_integrations::{SourceControlError, SourceControlProvider};
use tanren_store::{
    NewProject, NewProjectRepository, ProjectSetupRecord, ProjectStore, ProjectStoreError,
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
    /// Designated host where the repository should be created.
    pub designated_host: String,
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

    let setup = register_project_repository(
        store,
        command.request.owning_account_id,
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
    let designated_host = command.designated_host.trim();
    if designated_host.is_empty() {
        return Err(AppServiceError::Project(
            ProjectFailureReason::ValidationFailed,
        ));
    }

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
        created_repository,
        command.request.select_as_active,
        clock,
    )
    .await?;

    Ok(CreateProjectResponse {
        project: project_view(&setup),
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
    repository: RepositoryRef,
    select_as_active: bool,
    clock: &Clock,
) -> Result<ProjectSetupRecord, AppServiceError>
where
    S: ProjectStore + ?Sized,
{
    if store
        .find_project_repository(owning_account_id, &repository)
        .await?
        .is_some()
    {
        return Err(AppServiceError::Project(
            ProjectFailureReason::DuplicateRepository,
        ));
    }

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
                created_at: now,
            },
            select_as_active,
        )
        .await
        .map_err(map_project_store_error)?;

    Ok(setup)
}

fn map_provider_error(err: SourceControlError) -> AppServiceError {
    match err {
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
