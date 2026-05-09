use async_trait::async_trait;
use serde_json::Value;
use tanren_contract::{
    ActiveProjectRequest, ActiveProjectView, ConnectProjectRepositoryRequest,
    ConnectProjectRepositoryResponse, CreateProjectRequest, CreateProjectResponse,
    ListVisibleProjectsRequest, ProjectCollectionView, ProjectFailureReason,
};

use super::super::{HarnessError, HarnessResult, ProjectHarness};
use super::ApiHarness;

#[async_trait]
impl ProjectHarness for ApiHarness {
    async fn connect_project_repository(
        &mut self,
        req: ConnectProjectRepositoryRequest,
    ) -> HarnessResult<ConnectProjectRepositoryResponse> {
        let url = format!("{}/projects/connect-repository", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&req)
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
        let url = format!("{}/projects/list", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&req)
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
        let url = format!("{}/projects/create", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&req)
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
        let url = format!("{}/projects/active", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&req)
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
}

pub(crate) fn project_code_to_reason(code: &str) -> Option<ProjectFailureReason> {
    Some(match code {
        "duplicate_repository" => ProjectFailureReason::DuplicateRepository,
        "no_access" => ProjectFailureReason::NoAccess,
        "validation_failed" => ProjectFailureReason::ValidationFailed,
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
