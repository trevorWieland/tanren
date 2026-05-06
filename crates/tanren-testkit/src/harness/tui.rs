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
    AcceptInvitationRequest, ActiveProjectView, ConnectProjectRequest, CreateProjectRequest,
    ProjectView, SignInRequest, SignUpRequest,
};
use tanren_identity_policy::AccountId;
use tanren_store::EventEnvelope;

use super::in_process::InProcessHarness;
use super::{
    AccountHarness, HarnessAcceptance, HarnessInvitation, HarnessKind, HarnessResult,
    HarnessSession,
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

    async fn connect_project(
        &mut self,
        account_id: AccountId,
        request: ConnectProjectRequest,
    ) -> HarnessResult<ProjectView> {
        self.inner.connect_project(account_id, request).await
    }

    async fn create_project(
        &mut self,
        account_id: AccountId,
        request: CreateProjectRequest,
    ) -> HarnessResult<ProjectView> {
        self.inner.create_project(account_id, request).await
    }

    async fn active_project(
        &mut self,
        account_id: AccountId,
    ) -> HarnessResult<Option<ActiveProjectView>> {
        self.inner.active_project(account_id).await
    }

    async fn seed_accessible_repository(&mut self, host: &str, url: &str) -> HarnessResult<()> {
        self.inner.seed_accessible_repository(host, url).await
    }

    async fn seed_inaccessible_host(&mut self, host: &str) -> HarnessResult<()> {
        self.inner.seed_inaccessible_host(host).await
    }

    async fn seed_inaccessible_repository(&mut self, url: &str) -> HarnessResult<()> {
        self.inner.seed_inaccessible_repository(url).await
    }

    async fn seed_accessible_host(&mut self, host: &str) -> HarnessResult<()> {
        self.inner.seed_accessible_host(host).await
    }

    async fn seed_provider_not_configured(&mut self) -> HarnessResult<()> {
        self.inner.seed_provider_not_configured().await
    }
}
