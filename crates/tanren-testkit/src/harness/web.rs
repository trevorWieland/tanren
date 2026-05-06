//! `@web` harness — currently delegates to [`super::InProcessHarness`].
//!
//! PR 11 stands up a parallel Node-side Playwright harness for the same
//! `@web` Gherkin scenarios via `playwright-bdd`. The Gherkin source is
//! shared (the directory `apps/web/tests/bdd/features` is a symlink into
//! `tests/bdd/features/`), so the same scenarios prove themselves twice:
//!
//! - **Rust BDD** (this crate, fast feedback): every `@web` scenario
//!   routes through this harness, which falls back to the in-process
//!   `Handlers` dispatch.
//! - **playwright-bdd** (`apps/web/tests/bdd/`, real browser): the same
//!   `@web` scenarios run end-to-end against a Playwright-driven Chromium
//!   that hits a Next.js dev server pointed at a freshly spawned
//!   `tanren-api` binary.
//!
//! The two layers are not redundant: the Rust path proves wiring inside
//! the workspace at unit-test latency, and the Playwright path proves
//! the rendered DOM, the cookie round-trip, and the CORS-allowed origin.

use async_trait::async_trait;
use serde_json::Value;
use tanren_contract::{
    AcceptInvitationRequest, AttentionSpecView, ProjectScopedViews, ProjectView, SignInRequest,
    SignUpRequest, SwitchProjectResponse,
};
use tanren_identity_policy::{AccountId, ProjectId, SpecId};
use tanren_store::EventEnvelope;

use super::in_process::InProcessHarness;
use super::project::{HarnessProjectFixture, HarnessSpecFixture, ProjectHarness};
use super::{
    AccountHarness, HarnessAcceptance, HarnessInvitation, HarnessKind, HarnessResult,
    HarnessSession,
};

/// `@web` harness — fallback wrapper around [`InProcessHarness`]. The
/// real-browser proof lives on the Node side via `playwright-bdd`; this
/// harness keeps the Rust BDD runner self-contained for fast feedback.
#[derive(Debug)]
pub struct WebHarness {
    inner: InProcessHarness,
}

impl WebHarness {
    /// Construct the Web fallback harness.
    pub async fn spawn() -> HarnessResult<Self> {
        Ok(Self {
            inner: InProcessHarness::new(HarnessKind::Web).await?,
        })
    }
}

#[async_trait]
impl AccountHarness for WebHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Web
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
impl ProjectHarness for WebHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Web
    }

    async fn seed_project(&mut self, fixture: HarnessProjectFixture) -> HarnessResult<ProjectId> {
        self.inner.seed_project(fixture).await
    }

    async fn seed_spec(&mut self, fixture: HarnessSpecFixture) -> HarnessResult<SpecId> {
        self.inner.seed_spec(fixture).await
    }

    async fn seed_view_state(
        &mut self,
        account_id: AccountId,
        project_id: ProjectId,
        state: Value,
    ) -> HarnessResult<()> {
        self.inner
            .seed_view_state(account_id, project_id, state)
            .await
    }

    async fn list_projects(&mut self, account_id: AccountId) -> HarnessResult<Vec<ProjectView>> {
        self.inner.list_projects(account_id).await
    }

    async fn switch_active_project(
        &mut self,
        account_id: AccountId,
        project_id: ProjectId,
    ) -> HarnessResult<SwitchProjectResponse> {
        self.inner
            .switch_active_project(account_id, project_id)
            .await
    }

    async fn attention_spec(
        &mut self,
        account_id: AccountId,
        project_id: ProjectId,
        spec_id: SpecId,
    ) -> HarnessResult<AttentionSpecView> {
        self.inner
            .attention_spec(account_id, project_id, spec_id)
            .await
    }

    async fn project_scoped_views(
        &mut self,
        account_id: AccountId,
    ) -> HarnessResult<ProjectScopedViews> {
        self.inner.project_scoped_views(account_id).await
    }
}
