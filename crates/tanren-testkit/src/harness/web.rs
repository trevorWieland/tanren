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
    AcceptInvitationRequest, ActiveProjectRequest, ActiveProjectView,
    ConnectProjectRepositoryRequest, ConnectProjectRepositoryResponse, CreateProjectRequest,
    CreateProjectResponse, ListVisibleProjectsRequest, ProjectCollectionView, ProjectFailureReason,
    ProjectPageRequest, SignInRequest, SignUpRequest,
};
use tanren_identity_policy::{AccountId, DesignatedHost, RepositoryRef};
use tanren_store::EventEnvelope;

use super::HarnessError;
use super::in_process::InProcessHarness;
use super::{
    AccountHarness, HarnessAcceptance, HarnessInvitation, HarnessKind, HarnessResult,
    HarnessSession, ProjectHarness,
};

/// `@web` harness — fallback wrapper around [`InProcessHarness`]. The
/// real-browser proof lives on the Node side via `playwright-bdd`; this
/// harness keeps the Rust BDD runner self-contained for fast feedback.
#[derive(Debug)]
pub struct WebHarness {
    inner: InProcessHarness,
    session_account_id: Option<AccountId>,
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
            session_account_id: None,
        })
    }

    fn session_account_id(&self) -> HarnessResult<AccountId> {
        self.session_account_id.ok_or_else(|| {
            HarnessError::Project(
                ProjectFailureReason::AuthRequired,
                ProjectFailureReason::AuthRequired.code().to_owned(),
            )
        })
    }

    fn guard_legacy_scope(
        owning_account_id: AccountId,
        session_account_id: AccountId,
    ) -> HarnessResult<()> {
        if owning_account_id != session_account_id {
            return Err(HarnessError::Project(
                ProjectFailureReason::NoAccess,
                ProjectFailureReason::NoAccess.code().to_owned(),
            ));
        }
        Ok(())
    }
}

#[async_trait]
impl AccountHarness for WebHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Web
    }

    async fn sign_up(&mut self, req: SignUpRequest) -> HarnessResult<HarnessSession> {
        let session = self.inner.sign_up(req).await?;
        self.session_account_id = Some(session.account_id);
        Ok(session)
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        let session = self.inner.sign_in(req).await?;
        self.session_account_id = Some(session.account_id);
        Ok(session)
    }

    async fn accept_invitation(
        &mut self,
        req: AcceptInvitationRequest,
    ) -> HarnessResult<HarnessAcceptance> {
        let acceptance = self.inner.accept_invitation(req).await?;
        self.session_account_id = Some(acceptance.session.account_id);
        Ok(acceptance)
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
    async fn connect_project_repository(
        &mut self,
        req: ConnectProjectRepositoryRequest,
    ) -> HarnessResult<ConnectProjectRepositoryResponse> {
        let session_account_id = self.session_account_id()?;
        Self::guard_legacy_scope(req.owning_account_id, session_account_id)?;
        self.inner
            .connect_project_repository(ConnectProjectRepositoryRequest {
                owning_account_id: session_account_id,
                repository: req.repository,
                select_as_active: req.select_as_active,
            })
            .await
    }

    async fn list_visible_projects(
        &mut self,
        req: ListVisibleProjectsRequest,
    ) -> HarnessResult<ProjectCollectionView> {
        let session_account_id = self.session_account_id()?;
        Self::guard_legacy_scope(req.owning_account_id, session_account_id)?;
        self.inner
            .list_visible_projects(ListVisibleProjectsRequest {
                owning_account_id: session_account_id,
                page: ProjectPageRequest::default(),
            })
            .await
    }

    async fn create_project(
        &mut self,
        req: CreateProjectRequest,
    ) -> HarnessResult<CreateProjectResponse> {
        let session_account_id = self.session_account_id()?;
        Self::guard_legacy_scope(req.owning_account_id, session_account_id)?;
        self.inner
            .create_project(CreateProjectRequest {
                owning_account_id: session_account_id,
                repository: req.repository,
                designated_host: req.designated_host,
                select_as_active: req.select_as_active,
            })
            .await
    }

    async fn active_project(
        &mut self,
        req: ActiveProjectRequest,
    ) -> HarnessResult<ActiveProjectView> {
        let session_account_id = self.session_account_id()?;
        Self::guard_legacy_scope(req.owning_account_id, session_account_id)?;
        self.inner
            .active_project(ActiveProjectRequest {
                owning_account_id: session_account_id,
            })
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
}
