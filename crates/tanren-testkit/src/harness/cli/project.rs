use async_trait::async_trait;
use sea_orm::ConnectionTrait;
use tanren_app_services::project::{
    ActiveProjectQuery, ConnectExistingRepositoryCommand, CreateNewProjectCommand,
    ListVisibleProjectsQuery,
};
use tanren_app_services::{AppServiceError, Handlers};
use tanren_contract::{
    ActiveProjectRequest, ActiveProjectView, ConnectProjectRepositoryRequest,
    ConnectProjectRepositoryResponse, CreateProjectRequest, CreateProjectResponse,
    ListVisibleProjectsRequest, ProjectCollectionView,
};
use tanren_identity_policy::{AccountId, DesignatedHost, RepositoryRef};
use tanren_provider_integrations::{
    SourceControlProvider, fixture_source_control_provider_from_env_value,
};

use super::super::{HarnessError, HarnessResult, ProjectHarness};
use super::CliHarness;

#[async_trait]
impl ProjectHarness for CliHarness {
    async fn connect_project_repository(
        &mut self,
        req: ConnectProjectRepositoryRequest,
    ) -> HarnessResult<ConnectProjectRepositoryResponse> {
        self.connect_project_repository_as_actor(req.owning_account_id, req)
            .await
    }

    async fn connect_project_repository_as_actor(
        &mut self,
        actor_account_id: AccountId,
        req: ConnectProjectRepositoryRequest,
    ) -> HarnessResult<ConnectProjectRepositoryResponse> {
        let _session_token = session_token_for_actor(self, actor_account_id)?;
        let handlers = Handlers::new();
        let provider = cli_fixture_provider(self)?;
        handlers
            .connect_project_repository(
                self.store.as_ref(),
                provider.as_ref(),
                ConnectExistingRepositoryCommand {
                    actor_account_id,
                    request: req,
                },
            )
            .await
            .map_err(project_error_from_app_service)
    }

    async fn list_visible_projects(
        &mut self,
        req: ListVisibleProjectsRequest,
    ) -> HarnessResult<ProjectCollectionView> {
        self.list_visible_projects_as_actor(req.owning_account_id, req)
            .await
    }

    async fn list_visible_projects_as_actor(
        &mut self,
        actor_account_id: AccountId,
        req: ListVisibleProjectsRequest,
    ) -> HarnessResult<ProjectCollectionView> {
        let _session_token = session_token_for_actor(self, actor_account_id)?;
        let handlers = Handlers::new();
        handlers
            .list_visible_projects(
                self.store.as_ref(),
                ListVisibleProjectsQuery {
                    actor_account_id,
                    request: req,
                },
            )
            .await
            .map_err(project_error_from_app_service)
    }

    async fn create_project(
        &mut self,
        req: CreateProjectRequest,
    ) -> HarnessResult<CreateProjectResponse> {
        self.create_project_as_actor(req.owning_account_id, req)
            .await
    }

    async fn create_project_as_actor(
        &mut self,
        actor_account_id: AccountId,
        req: CreateProjectRequest,
    ) -> HarnessResult<CreateProjectResponse> {
        if !req.select_as_active {
            return Err(HarnessError::Transport(
                "cli harness currently expects select_as_active=true".to_owned(),
            ));
        }
        let _session_token = session_token_for_actor(self, actor_account_id)?;
        let handlers = Handlers::new();
        let provider = cli_fixture_provider(self)?;
        let designated_host = req.designated_host.clone();
        let response = handlers
            .create_project(
                self.store.as_ref(),
                provider.as_ref(),
                CreateNewProjectCommand {
                    actor_account_id,
                    request: req,
                },
            )
            .await
            .map_err(project_error_from_app_service)?;
        self.record_created_repository(&designated_host, &response.project.repository.repository);
        Ok(response)
    }

    async fn active_project(
        &mut self,
        req: ActiveProjectRequest,
    ) -> HarnessResult<ActiveProjectView> {
        self.active_project_as_actor(req.owning_account_id, req)
            .await
    }

    async fn active_project_as_actor(
        &mut self,
        actor_account_id: AccountId,
        req: ActiveProjectRequest,
    ) -> HarnessResult<ActiveProjectView> {
        let _session_token = session_token_for_actor(self, actor_account_id)?;
        let handlers = Handlers::new();
        handlers
            .active_project(
                self.store.as_ref(),
                ActiveProjectQuery {
                    actor_account_id,
                    request: req,
                },
            )
            .await
            .map_err(project_error_from_app_service)
    }

    async fn set_repository_access(
        &mut self,
        actor_account_id: AccountId,
        repository: RepositoryRef,
        allowed: bool,
    ) -> HarnessResult<()> {
        self.configure_repository_access(actor_account_id, repository, allowed);
        Ok(())
    }

    async fn set_designated_host_create_access(
        &mut self,
        actor_account_id: AccountId,
        host: DesignatedHost,
        allowed: bool,
    ) -> HarnessResult<()> {
        self.configure_designated_host_create_access(actor_account_id, &host, allowed);
        Ok(())
    }

    async fn repository_created_at_host(
        &self,
        host: &DesignatedHost,
        repository: &RepositoryRef,
    ) -> HarnessResult<bool> {
        Ok(self.observed_repository_created_at_host(host, repository))
    }

    async fn break_project_store_for_testing(&mut self) -> HarnessResult<()> {
        self.store
            .connection()
            .execute_unprepared("DROP TABLE IF EXISTS projects")
            .await
            .map_err(|e| HarnessError::Transport(format!("drop projects table: {e}")))?;
        Ok(())
    }
}

fn project_error_from_app_service(err: AppServiceError) -> HarnessError {
    match err {
        AppServiceError::Project(reason) => HarnessError::Project(reason, reason.summary().to_owned()),
        AppServiceError::InvalidInput(message) => {
            HarnessError::Transport(format!("validation_failed: {message}"))
        }
        _ => HarnessError::Transport(
            "internal_error: Tanren encountered an internal error while processing the project request."
                .to_owned(),
        ),
    }
}

fn session_token_for_actor(
    harness: &CliHarness,
    actor_account_id: AccountId,
) -> HarnessResult<String> {
    harness
        .session_token_for(actor_account_id)
        .map(str::to_owned)
        .ok_or_else(|| {
            HarnessError::Transport(format!(
                "missing CLI session token for actor account {actor_account_id}"
            ))
        })
}

fn cli_fixture_provider(
    harness: &CliHarness,
) -> HarnessResult<std::sync::Arc<dyn SourceControlProvider>> {
    let raw = harness.project_provider_fixture_env_value();
    fixture_source_control_provider_from_env_value(&raw).ok_or_else(|| {
        HarnessError::Transport(
            "internal_error: failed to build CLI fixture source-control provider".to_owned(),
        )
    })
}
