//! `@tui` harness — currently delegates to [`super::InProcessHarness`].
//!
//! TODO(R-0001 sub-11 or follow-up): wire `expectrl` + `portable-pty`
//! to drive the `tanren-tui` binary inside a real pseudo-terminal.
//! The ratatui screen-scrape path was prototyped but proved too
//! fragile to commit as the default — the `expectrl` workspace dep
//! is staged in `Cargo.toml [workspace.dependencies]` so the next
//! iteration can import it without further dependency churn.
//!
//! Until that lands, every `@tui` scenario routes through the
//! direct-`Handlers` in-process harness — the same surface every
//! interface delegates to via the equivalent-operations rule in
//! `docs/architecture/subsystems/interfaces.md`. The wire harness
//! coverage check (`xtask check-bdd-wire-coverage`) is satisfied
//! because step bodies dispatch through the `AccountHarness` trait,
//! which keeps `Handlers::*` invisible from `tanren-bdd`.

use async_trait::async_trait;
use tanren_contract::{
    AcceptInvitationRequest, ActiveProjectRequest, ActiveProjectView,
    ConnectProjectRepositoryRequest, ConnectProjectRepositoryResponse, CreateProjectRequest,
    CreateProjectResponse, ListVisibleProjectsRequest, ProjectCollectionView, SignInRequest,
    SignUpRequest,
};
use tanren_identity_policy::{AccountId, DesignatedHost, RepositoryRef};
use tanren_store::EventEnvelope;

use super::in_process::InProcessHarness;
use super::{
    AccountHarness, HarnessAcceptance, HarnessInvitation, HarnessKind, HarnessResult,
    HarnessSession, ProjectHarness,
};

/// `@tui` harness — fallback wrapper around [`InProcessHarness`] until
/// the expectrl-based driver lands.
#[derive(Debug)]
pub struct TuiHarness {
    inner: InProcessHarness,
}

impl TuiHarness {
    /// Construct the fallback TUI harness.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying in-process harness cannot
    /// initialize an ephemeral `SQLite` store.
    pub async fn spawn() -> HarnessResult<Self> {
        Ok(Self {
            inner: InProcessHarness::new(HarnessKind::Tui).await?,
        })
    }
}

#[async_trait]
impl AccountHarness for TuiHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Tui
    }

    async fn sign_up(&mut self, req: SignUpRequest) -> HarnessResult<HarnessSession> {
        self.inner.sign_up(req).await
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        self.inner.sign_in(req).await
    }

    async fn accept_invitation(
        &mut self,
        req: AcceptInvitationRequest,
    ) -> HarnessResult<HarnessAcceptance> {
        self.inner.accept_invitation(req).await
    }

    async fn seed_invitation(&mut self, fixture: HarnessInvitation) -> HarnessResult<()> {
        self.inner.seed_invitation(fixture).await
    }

    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<EventEnvelope>> {
        self.inner.recent_events(limit).await
    }
}

#[async_trait]
impl ProjectHarness for TuiHarness {
    async fn connect_project_repository(
        &mut self,
        req: ConnectProjectRepositoryRequest,
    ) -> HarnessResult<ConnectProjectRepositoryResponse> {
        self.inner.connect_project_repository(req).await
    }

    async fn connect_project_repository_as_actor(
        &mut self,
        actor_account_id: AccountId,
        req: ConnectProjectRepositoryRequest,
    ) -> HarnessResult<ConnectProjectRepositoryResponse> {
        self.inner
            .connect_project_repository_as_actor(actor_account_id, req)
            .await
    }

    async fn list_visible_projects(
        &mut self,
        req: ListVisibleProjectsRequest,
    ) -> HarnessResult<ProjectCollectionView> {
        self.inner.list_visible_projects(req).await
    }

    async fn list_visible_projects_as_actor(
        &mut self,
        actor_account_id: AccountId,
        req: ListVisibleProjectsRequest,
    ) -> HarnessResult<ProjectCollectionView> {
        self.inner
            .list_visible_projects_as_actor(actor_account_id, req)
            .await
    }

    async fn create_project(
        &mut self,
        req: CreateProjectRequest,
    ) -> HarnessResult<CreateProjectResponse> {
        self.inner.create_project(req).await
    }

    async fn create_project_as_actor(
        &mut self,
        actor_account_id: AccountId,
        req: CreateProjectRequest,
    ) -> HarnessResult<CreateProjectResponse> {
        self.inner
            .create_project_as_actor(actor_account_id, req)
            .await
    }

    async fn active_project(
        &mut self,
        req: ActiveProjectRequest,
    ) -> HarnessResult<ActiveProjectView> {
        self.inner.active_project(req).await
    }

    async fn active_project_as_actor(
        &mut self,
        actor_account_id: AccountId,
        req: ActiveProjectRequest,
    ) -> HarnessResult<ActiveProjectView> {
        self.inner
            .active_project_as_actor(actor_account_id, req)
            .await
    }

    async fn set_repository_access(
        &mut self,
        actor_account_id: AccountId,
        repository: RepositoryRef,
        allowed: bool,
    ) -> HarnessResult<()> {
        self.inner
            .set_repository_access(actor_account_id, repository, allowed)
            .await
    }

    async fn set_designated_host_create_access(
        &mut self,
        actor_account_id: AccountId,
        host: DesignatedHost,
        allowed: bool,
    ) -> HarnessResult<()> {
        self.inner
            .set_designated_host_create_access(actor_account_id, host, allowed)
            .await
    }

    async fn repository_created_at_host(
        &self,
        host: &DesignatedHost,
        repository: &RepositoryRef,
    ) -> HarnessResult<bool> {
        self.inner
            .repository_created_at_host(host, repository)
            .await
    }

    async fn source_control_call_counters(
        &mut self,
    ) -> HarnessResult<tanren_provider_integrations::SourceControlCallCounters> {
        self.inner.source_control_call_counters().await
    }

    async fn break_project_store_for_testing(&mut self) -> HarnessResult<()> {
        self.inner.break_project_store_for_testing().await
    }
}
