use async_trait::async_trait;
use chrono::Utc;
use tanren_app_services::ActorContext;
use tanren_app_services::project::{ProjectDependencyView, ProjectSpecView};
use tanren_contract::{
    ConnectProjectRequest, ConnectProjectResponse, DisconnectProjectRequest,
    DisconnectProjectResponse, ListProjectsResponse, ReconnectProjectResponse,
};
use tanren_identity_policy::{AccountId, OrgId, ProjectId, ProviderConnectionId, SpecId};
use tanren_store::EventEnvelope;

use super::project::{ProjectHarness, translate_project_error};
use super::tui::TuiHarness;
use super::{HarnessError, HarnessKind, HarnessResult};

pub struct ProjectTuiHarness {
    inner: TuiHarness,
    account_id: Option<AccountId>,
}

impl std::fmt::Debug for ProjectTuiHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectTuiHarness").finish_non_exhaustive()
    }
}

impl ProjectTuiHarness {
    pub async fn spawn() -> HarnessResult<Self> {
        Ok(Self {
            inner: TuiHarness::spawn().await?,
            account_id: None,
        })
    }
}

fn make_actor(account_id: AccountId) -> ActorContext {
    ActorContext::from_account_id(account_id)
}

#[async_trait]
impl ProjectHarness for ProjectTuiHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Tui
    }

    fn provider_connection_id(&self) -> ProviderConnectionId {
        self.inner.provider_connection_id()
    }

    async fn connect_project(
        &mut self,
        req: ConnectProjectRequest,
    ) -> HarnessResult<ConnectProjectResponse> {
        let account_id = self.account_id.expect("seed_account must be called first");
        let actor = make_actor(account_id);
        self.inner
            .handlers()
            .connect_project(self.inner.store_handle(), &actor, req)
            .await
            .map_err(translate_project_error)
    }

    async fn disconnect_project(
        &mut self,
        req: DisconnectProjectRequest,
    ) -> HarnessResult<DisconnectProjectResponse> {
        let account_id = self.account_id.expect("seed_account must be called first");
        let actor = make_actor(account_id);
        self.inner
            .handlers()
            .disconnect_project(self.inner.store_handle(), &actor, req)
            .await
            .map_err(translate_project_error)
    }

    async fn list_projects(
        &mut self,
        account_id: AccountId,
    ) -> HarnessResult<ListProjectsResponse> {
        let actor = make_actor(account_id);
        self.inner
            .handlers()
            .list_projects(self.inner.store_handle(), &actor)
            .await
            .map_err(translate_project_error)
    }

    async fn reconnect_project(
        &mut self,
        project_id: ProjectId,
    ) -> HarnessResult<ReconnectProjectResponse> {
        let account_id = self.account_id.expect("seed_account must be called first");
        let actor = make_actor(account_id);
        self.inner
            .handlers()
            .reconnect_project(self.inner.store_handle(), &actor, project_id)
            .await
            .map_err(translate_project_error)
    }

    async fn project_specs(
        &mut self,
        project_id: ProjectId,
    ) -> HarnessResult<Vec<ProjectSpecView>> {
        let account_id = self.account_id.expect("seed_account must be called first");
        let actor = make_actor(account_id);
        self.inner
            .handlers()
            .project_specs(self.inner.store_handle(), &actor, project_id)
            .await
            .map_err(translate_project_error)
    }

    async fn project_dependencies(
        &mut self,
        project_id: ProjectId,
    ) -> HarnessResult<Vec<ProjectDependencyView>> {
        let account_id = self.account_id.expect("seed_account must be called first");
        let actor = make_actor(account_id);
        self.inner
            .handlers()
            .project_dependencies(self.inner.store_handle(), &actor, project_id)
            .await
            .map_err(translate_project_error)
    }

    async fn seed_account(&mut self) -> HarnessResult<(AccountId, OrgId)> {
        let aid = AccountId::fresh();
        let oid = OrgId::fresh();
        self.inner
            .store_handle_mut()
            .seed_account_with_org(aid, oid, Utc::now())
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_account: {e}")))?;
        self.account_id = Some(aid);
        Ok((aid, oid))
    }

    async fn seed_spec(&mut self, project_id: ProjectId, title: String) -> HarnessResult<SpecId> {
        let sid = SpecId::fresh();
        self.inner
            .store_handle_mut()
            .seed_project_spec(sid, project_id, title, String::new(), Utc::now())
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_spec: {e}")))?;
        Ok(sid)
    }

    async fn seed_dependency(
        &mut self,
        source: ProjectId,
        source_spec: SpecId,
        target: ProjectId,
    ) -> HarnessResult<()> {
        self.inner
            .store_handle_mut()
            .seed_project_dependency(source, source_spec, target, Utc::now())
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_dependency: {e}")))?;
        Ok(())
    }

    async fn seed_active_loop(&mut self, project_id: ProjectId) -> HarnessResult<()> {
        self.inner
            .store_handle_mut()
            .seed_loop_fixture(project_id, true, Utc::now())
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_active_loop: {e}")))?;
        Ok(())
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
