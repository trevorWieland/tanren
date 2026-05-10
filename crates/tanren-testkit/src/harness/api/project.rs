use async_trait::async_trait;
use sea_orm::ConnectionTrait;
use serde_json::Value;
use tanren_contract::{
    ActiveProjectCookieRequest, ActiveProjectRequest, ActiveProjectView,
    ConnectProjectRepositoryCookieRequest, ConnectProjectRepositoryRequest,
    ConnectProjectRepositoryResponse, CreateProjectCookieRequest, CreateProjectRequest,
    CreateProjectResponse, ListVisibleProjectsCookieRequest, ListVisibleProjectsRequest,
    ProjectCollectionView, ProjectFailureReason,
};
use tanren_identity_policy::{AccountId, DesignatedHost, RepositoryRef};

use super::super::{HarnessError, HarnessResult, ProjectHarness};
use super::ApiHarness;

#[async_trait]
impl ProjectHarness for ApiHarness {
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
        if actor_account_id != req.owning_account_id {
            let reason = ProjectFailureReason::NoAccess;
            return Err(HarnessError::Project(reason, reason.summary().to_owned()));
        }
        let url = format!("{}/projects/connect-repository", self.base_url);
        let body = ConnectProjectRepositoryCookieRequest {
            repository: req.repository,
            select_as_active: req.select_as_active,
        };
        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                HarnessError::Transport(format!("POST /projects/connect-repository: {e}"))
            })?;
        let status = response.status();
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(project_failure_from_body(&json));
        }
        serde_json::from_value(json)
            .map_err(|e| HarnessError::Transport(format!("decode connect project response: {e}")))
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
        if actor_account_id != req.owning_account_id {
            let reason = ProjectFailureReason::NoAccess;
            return Err(HarnessError::Project(reason, reason.summary().to_owned()));
        }
        let url = format!("{}/projects/list", self.base_url);
        let body = ListVisibleProjectsCookieRequest { page: req.page };
        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("POST /projects/list: {e}")))?;
        let status = response.status();
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(project_failure_from_body(&json));
        }
        serde_json::from_value(json)
            .map_err(|e| HarnessError::Transport(format!("decode list projects response: {e}")))
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
        if actor_account_id != req.owning_account_id {
            let reason = ProjectFailureReason::NoAccess;
            return Err(HarnessError::Project(reason, reason.summary().to_owned()));
        }
        let url = format!("{}/projects/create", self.base_url);
        let body = CreateProjectCookieRequest {
            repository: req.repository,
            designated_host: req.designated_host,
            select_as_active: req.select_as_active,
        };
        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("POST /projects/create: {e}")))?;
        let status = response.status();
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(project_failure_from_body(&json));
        }
        serde_json::from_value(json)
            .map_err(|e| HarnessError::Transport(format!("decode create project response: {e}")))
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
        if actor_account_id != req.owning_account_id {
            let reason = ProjectFailureReason::NoAccess;
            return Err(HarnessError::Project(reason, reason.summary().to_owned()));
        }
        let url = format!("{}/projects/active", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&ActiveProjectCookieRequest::default())
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("POST /projects/active: {e}")))?;
        let status = response.status();
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(project_failure_from_body(&json));
        }
        serde_json::from_value(json)
            .map_err(|e| HarnessError::Transport(format!("decode active project response: {e}")))
    }

    async fn set_repository_access(
        &mut self,
        actor_account_id: AccountId,
        repository: RepositoryRef,
        allowed: bool,
    ) -> HarnessResult<()> {
        self.fixture_source_control
            .set_repository_access(actor_account_id, repository, allowed);
        Ok(())
    }

    async fn set_designated_host_create_access(
        &mut self,
        actor_account_id: AccountId,
        host: DesignatedHost,
        allowed: bool,
    ) -> HarnessResult<()> {
        self.fixture_source_control.set_host_reachable(&host, true);
        self.fixture_source_control
            .set_host_create_access(actor_account_id, &host, allowed);
        Ok(())
    }

    async fn repository_created_at_host(
        &self,
        host: &DesignatedHost,
        repository: &RepositoryRef,
    ) -> HarnessResult<bool> {
        Ok(self
            .fixture_source_control
            .repository_created_at_host(host, repository))
    }

    async fn source_control_call_counters(
        &mut self,
    ) -> HarnessResult<tanren_provider_integrations::SourceControlCallCounters> {
        Ok(self.fixture_source_control.call_counters())
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

pub(crate) fn project_code_to_reason(code: &str) -> Option<ProjectFailureReason> {
    Some(match code {
        "auth_required" => ProjectFailureReason::AuthRequired,
        "duplicate_repository" => ProjectFailureReason::DuplicateRepository,
        "no_access" => ProjectFailureReason::NoAccess,
        "validation_failed" => ProjectFailureReason::ValidationFailed,
        "provider_unavailable" => ProjectFailureReason::ProviderUnavailable,
        "provider_failure" => ProjectFailureReason::ProviderFailure,
        _ => return None,
    })
}

fn project_failure_from_body(json: &Value) -> HarnessError {
    let code = json
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or("transport_error")
        .to_owned();
    let summary = json
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("unknown failure")
        .to_owned();
    if let Some(reason) = project_code_to_reason(&code) {
        HarnessError::Project(reason, summary)
    } else {
        HarnessError::Transport(format!("{code}: {summary}"))
    }
}
