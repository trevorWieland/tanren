use async_trait::async_trait;
use serde_json::Value;
use tanren_app_services::project::{ProjectDependencyView, ProjectSpecView};
use tanren_contract::{
    ConnectProjectRequest, ConnectProjectResponse, DisconnectProjectRequest,
    DisconnectProjectResponse, ListProjectsResponse, ProjectView, ReconnectProjectResponse,
};
use tanren_identity_policy::{AccountId, OrgId, ProjectId, SpecId};
use tanren_store::EventEnvelope;

use super::api::ApiHarness;
use super::project::{
    ProjectHarness, record_to_view, seed_account_via_store, seed_active_loop_via_store,
    seed_dependency_via_store, seed_spec_via_store,
};
use super::{HarnessError, HarnessKind, HarnessResult};

pub struct ProjectApiHarness {
    inner: ApiHarness,
}

impl std::fmt::Debug for ProjectApiHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectApiHarness").finish_non_exhaustive()
    }
}

impl ProjectApiHarness {
    pub async fn spawn() -> HarnessResult<Self> {
        Ok(Self {
            inner: ApiHarness::spawn().await?,
        })
    }

    async fn project_json(
        &self,
        method: &str,
        url: String,
        body: Option<Value>,
    ) -> HarnessResult<Value> {
        let client = self.inner.http_client().clone();
        let req = match method {
            "POST" => client.post(&url),
            "GET" => client.get(&url),
            _ => {
                return Err(HarnessError::Transport(format!(
                    "unsupported method: {method}"
                )));
            }
        };
        let req = match body {
            Some(b) => req.json(&b),
            None => req,
        };
        let response = req
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("{method} {url}: {e}")))?;
        let status = response.status();
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() && status.as_u16() != 201 {
            return Err(super::api::failure_from_body(&json));
        }
        Ok(json)
    }
}

#[async_trait]
impl ProjectHarness for ProjectApiHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Api
    }

    async fn connect_project(
        &mut self,
        req: ConnectProjectRequest,
    ) -> HarnessResult<ConnectProjectResponse> {
        let url = format!("{}/projects", self.inner.base_url());
        let body = serde_json::to_value(&req)
            .map_err(|e| HarnessError::Transport(format!("serialize: {e}")))?;
        let json = self.project_json("POST", url, Some(body)).await?;
        let project: ProjectView = serde_json::from_value(json["project"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode project: {e}")))?;
        Ok(ConnectProjectResponse { project })
    }

    async fn disconnect_project(
        &mut self,
        req: DisconnectProjectRequest,
    ) -> HarnessResult<DisconnectProjectResponse> {
        let url = format!(
            "{}/projects/{}/disconnect",
            self.inner.base_url(),
            req.project_id
        );
        let body = serde_json::json!({ "account_id": req.account_id });
        let json = self.project_json("POST", url, Some(body)).await?;
        let resp: DisconnectProjectResponse = serde_json::from_value(json)
            .map_err(|e| HarnessError::Transport(format!("decode disconnect: {e}")))?;
        Ok(resp)
    }

    async fn list_projects(
        &mut self,
        account_id: AccountId,
    ) -> HarnessResult<ListProjectsResponse> {
        let url = format!(
            "{}/projects?account_id={}",
            self.inner.base_url(),
            account_id
        );
        let json = self.project_json("GET", url, None).await?;
        let resp: ListProjectsResponse = serde_json::from_value(json)
            .map_err(|e| HarnessError::Transport(format!("decode list: {e}")))?;
        Ok(resp)
    }

    async fn reconnect_project(
        &mut self,
        project_id: ProjectId,
    ) -> HarnessResult<ReconnectProjectResponse> {
        use tanren_store::ProjectStore as _;
        let reconnected = self
            .inner
            .store_handle()
            .reconnect_project(project_id)
            .await
            .map_err(|e| HarnessError::Transport(format!("reconnect: {e}")))?;
        Ok(ReconnectProjectResponse {
            project: record_to_view(&reconnected.project),
        })
    }

    async fn project_specs(
        &mut self,
        project_id: ProjectId,
    ) -> HarnessResult<Vec<ProjectSpecView>> {
        let url = format!("{}/projects/{}/specs", self.inner.base_url(), project_id);
        let json = self.project_json("GET", url, None).await?;
        let specs: Vec<ProjectSpecView> = json["specs"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();
        Ok(specs)
    }

    async fn project_dependencies(
        &mut self,
        project_id: ProjectId,
    ) -> HarnessResult<Vec<ProjectDependencyView>> {
        let url = format!(
            "{}/projects/{}/dependencies",
            self.inner.base_url(),
            project_id
        );
        let json = self.project_json("GET", url, None).await?;
        let deps: Vec<ProjectDependencyView> = json["dependencies"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();
        Ok(deps)
    }

    async fn seed_account(&mut self) -> HarnessResult<(AccountId, OrgId)> {
        seed_account_via_store(self.inner.store_handle()).await
    }

    async fn seed_spec(&mut self, project_id: ProjectId, title: String) -> HarnessResult<SpecId> {
        seed_spec_via_store(self.inner.store_handle(), project_id, title).await
    }

    async fn seed_dependency(
        &mut self,
        source: ProjectId,
        source_spec: SpecId,
        target: ProjectId,
    ) -> HarnessResult<()> {
        seed_dependency_via_store(self.inner.store_handle(), source, source_spec, target).await
    }

    async fn seed_active_loop(&mut self, project_id: ProjectId) -> HarnessResult<()> {
        seed_active_loop_via_store(self.inner.store_handle(), project_id).await
    }

    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<EventEnvelope>> {
        use tanren_store::AccountStore as _;
        self.inner
            .store_handle()
            .recent_events(limit)
            .await
            .map_err(|e| HarnessError::Transport(format!("recent_events: {e}")))
    }
}
