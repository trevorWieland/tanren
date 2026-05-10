//! `@web` harness — delegates to [`super::ApiHarness`].
//!
//! PR 11 stands up a parallel Node-side Playwright harness for the same
//! `@web` Gherkin scenarios via `playwright-bdd`. The Gherkin source is
//! shared (the directory `apps/web/tests/bdd/features` is a symlink into
//! `tests/bdd/features/`), so the same scenarios prove themselves twice:
//!
//! - **Rust BDD** (this crate, fast feedback): every `@web` scenario
//!   routes through this harness, which drives the same HTTP wire
//!   shape as `@api` while retaining `@web` scenario tags. The witness
//!   is the cucumber-rs run executed
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
use serde_json::Value;
use tanren_contract::{
    AcceptInvitationRequest, DeploymentPostureScope, SetDeploymentPostureRequest, SignInRequest,
    SignUpRequest,
};
use tanren_identity_policy::AccountId;
use tanren_store::EventEnvelope;

use super::api::ApiHarness;
use super::{
    AccountHarness, HarnessAcceptance, HarnessInvitation, HarnessKind, HarnessPostureView,
    HarnessResult, HarnessSession, HarnessSupportedPosture,
};

/// `@web` harness — wrapper around [`ApiHarness`]. The real-browser
/// proof lives on the Node side via `playwright-bdd`; this harness
/// keeps Rust BDD on real HTTP decoding semantics for posture mutations.
#[derive(Debug)]
pub struct WebHarness {
    inner: ApiHarness,
}

impl WebHarness {
    /// Construct the Web fallback harness.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying API harness cannot
    /// initialize an ephemeral `SQLite` store + HTTP app.
    pub async fn spawn() -> HarnessResult<Self> {
        Ok(Self {
            inner: ApiHarness::spawn().await?,
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

    async fn list_supported_postures(&mut self) -> HarnessResult<Vec<HarnessSupportedPosture>> {
        self.inner.list_supported_postures().await
    }

    async fn set_deployment_posture(
        &mut self,
        actor: AccountId,
        request: SetDeploymentPostureRequest,
    ) -> HarnessResult<HarnessPostureView> {
        self.inner.set_deployment_posture(actor, request).await
    }

    async fn set_deployment_posture_raw(
        &mut self,
        actor: AccountId,
        scope: DeploymentPostureScope,
        posture_raw: &str,
    ) -> HarnessResult<HarnessPostureView> {
        self.inner
            .set_deployment_posture_raw(actor, scope, posture_raw)
            .await
    }

    async fn set_deployment_posture_raw_scope(
        &mut self,
        actor: AccountId,
        scope_raw: Value,
        posture_raw: &str,
    ) -> HarnessResult<HarnessPostureView> {
        self.inner
            .set_deployment_posture_raw_scope(actor, scope_raw, posture_raw)
            .await
    }

    async fn get_deployment_posture(
        &mut self,
        actor: AccountId,
        scope: DeploymentPostureScope,
    ) -> HarnessResult<Option<HarnessPostureView>> {
        self.inner.get_deployment_posture(actor, scope).await
    }

    async fn seed_invitation(&mut self, fixture: HarnessInvitation) -> HarnessResult<()> {
        self.inner.seed_invitation(fixture).await
    }

    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<EventEnvelope>> {
        self.inner.recent_events(limit).await
    }
}
