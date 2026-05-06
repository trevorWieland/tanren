use async_trait::async_trait;
use chrono::Utc;
use secrecy::ExposeSecret;
use serde_json::Value;
use tanren_app_services::project::{ProjectDependencyView, ProjectSpecView};
use tanren_contract::{
    ConnectProjectRequest, ConnectProjectResponse, DisconnectProjectRequest,
    DisconnectProjectResponse, ListProjectsResponse, ReconnectProjectResponse,
};
use tanren_identity_policy::{AccountId, Email, OrgId, ProjectId, SpecId};
use tanren_store::{AccountStore as _, EventEnvelope, ProjectStore as _};

use super::mcp::McpHarness;
use super::project::{
    ProjectHarness, seed_active_loop_via_store, seed_dependency_via_store, seed_spec_via_store,
};
use super::{HarnessError, HarnessKind, HarnessResult};

pub struct ProjectMcpHarness {
    inner: McpHarness,
    account_id: Option<AccountId>,
}

impl std::fmt::Debug for ProjectMcpHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectMcpHarness").finish_non_exhaustive()
    }
}

impl ProjectMcpHarness {
    pub async fn spawn() -> HarnessResult<Self> {
        Ok(Self {
            inner: McpHarness::spawn().await?,
            account_id: None,
        })
    }

    async fn call_project_tool(&mut self, name: &'static str, body: Value) -> HarnessResult<Value> {
        self.inner.call_tool(name, body).await
    }
}

#[async_trait]
impl ProjectHarness for ProjectMcpHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Mcp
    }

    async fn connect_project(
        &mut self,
        req: ConnectProjectRequest,
    ) -> HarnessResult<ConnectProjectResponse> {
        let body = serde_json::to_value(&req)
            .map_err(|e| HarnessError::Transport(format!("serialize: {e}")))?;
        let payload = self.call_project_tool("project.connect", body).await?;
        let resp: ConnectProjectResponse = serde_json::from_value(payload)
            .map_err(|e| HarnessError::Transport(format!("decode connect: {e}")))?;
        Ok(resp)
    }

    async fn disconnect_project(
        &mut self,
        req: DisconnectProjectRequest,
    ) -> HarnessResult<DisconnectProjectResponse> {
        let body = serde_json::to_value(&req)
            .map_err(|e| HarnessError::Transport(format!("serialize: {e}")))?;
        let payload = self.call_project_tool("project.disconnect", body).await?;
        let resp: DisconnectProjectResponse = serde_json::from_value(payload)
            .map_err(|e| HarnessError::Transport(format!("decode disconnect: {e}")))?;
        Ok(resp)
    }

    async fn list_projects(
        &mut self,
        _account_id: AccountId,
    ) -> HarnessResult<ListProjectsResponse> {
        let body = serde_json::json!({});
        let payload = self.call_project_tool("project.list", body).await?;
        let resp: ListProjectsResponse = serde_json::from_value(payload)
            .map_err(|e| HarnessError::Transport(format!("decode list: {e}")))?;
        Ok(resp)
    }

    async fn reconnect_project(
        &mut self,
        project_id: ProjectId,
    ) -> HarnessResult<ReconnectProjectResponse> {
        let reconnected = self
            .inner
            .store_handle()
            .reconnect_project(project_id)
            .await
            .map_err(|e| HarnessError::Transport(format!("reconnect: {e}")))?;
        Ok(ReconnectProjectResponse {
            project: super::project::record_to_view(&reconnected.project),
        })
    }

    async fn project_specs(
        &mut self,
        project_id: ProjectId,
    ) -> HarnessResult<Vec<ProjectSpecView>> {
        let body = serde_json::json!({
            "project_id": project_id,
        });
        let payload = self.call_project_tool("project.specs", body).await?;
        let specs: Vec<ProjectSpecView> = serde_json::from_value(payload)
            .map_err(|e| HarnessError::Transport(format!("decode specs: {e}")))?;
        Ok(specs)
    }

    async fn project_dependencies(
        &mut self,
        project_id: ProjectId,
    ) -> HarnessResult<Vec<ProjectDependencyView>> {
        let body = serde_json::json!({
            "project_id": project_id,
        });
        let payload = self.call_project_tool("project.dependencies", body).await?;
        let deps: Vec<ProjectDependencyView> = serde_json::from_value(payload)
            .map_err(|e| HarnessError::Transport(format!("decode deps: {e}")))?;
        Ok(deps)
    }

    async fn seed_account(&mut self) -> HarnessResult<(AccountId, OrgId)> {
        let email_addr = format!(
            "project-harness-{}@example.com",
            uuid::Uuid::new_v4().simple()
        );
        let email = Email::parse(&email_addr)
            .map_err(|e| HarnessError::Transport(format!("parse email: {e}")))?;
        let password = secrecy::SecretString::from("harness-password-123456".to_owned());
        let body = serde_json::json!({
            "email": email.as_str(),
            "password": password.expose_secret(),
            "display_name": "Project Harness",
        });
        let payload = self.call_project_tool("account.create", body).await?;
        let account_id: AccountId = serde_json::from_value(payload["account"]["id"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode account.id: {e}")))?;

        let oid = OrgId::fresh();
        self.inner
            .store_handle()
            .insert_membership(account_id, oid, Utc::now())
            .await
            .map_err(|e| HarnessError::Transport(format!("insert membership: {e}")))?;

        self.account_id = Some(account_id);
        Ok((account_id, oid))
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
