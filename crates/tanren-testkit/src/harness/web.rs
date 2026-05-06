//! `@web` harness — currently delegates to [`super::InProcessHarness`].
//!
//! PR 11 stands up a parallel Node-side Playwright harness for the same
//! `@web` Gherkin scenarios via `playwright-bdd`. The Gherkin source is
//! shared (the directory `apps/web/tests/bdd/features` is a symlink into
//! `tests/bdd/features/`), so the same scenarios prove themselves twice:
//!
//! - **Rust BDD** (this crate, fast feedback): every `@web` scenario
//!   routes through this harness, which falls back to the in-process
//!   `Handlers` dispatch. The witness is the cucumber-rs run executed
//!   by `just tests` and `cargo run -p tanren-bdd --bin tanren-bdd-runner`.
//! - **playwright-bdd** (`apps/web/tests/bdd/`, real browser): the same
//!   `@web` scenarios run end-to-end against a Playwright-driven Chromium
//!   that hits a Next.js dev server pointed at a freshly spawned
//!   `tanren-api` binary. The witness is `pnpm --filter @tanren/web run e2e`,
//!   wired into `just web-test` (which `just ci` invokes).
//!
//! The two layers are not redundant: the Rust path proves wiring inside
//! the workspace at unit-test latency, and the Playwright path proves
//! the rendered DOM, the cookie round-trip, and the CORS-allowed origin.
//! See the dual-coverage note in `apps/web/tests/bdd/steps/account.steps.ts`.

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

/// `@web` harness — fallback wrapper around [`InProcessHarness`]. The
/// real-browser proof lives on the Node side via `playwright-bdd`; this
/// harness keeps the Rust BDD runner self-contained for fast feedback.
#[derive(Debug)]
pub struct WebHarness {
    inner: InProcessHarness,
}

impl WebHarness {
    /// Construct the Web fallback harness.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying in-process harness cannot
    /// initialize an ephemeral `SQLite` store.
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
