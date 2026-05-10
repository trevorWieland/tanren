//! Direct-`Handlers` harness — the legacy path the rest of the test
//! suite ran on before R-0001 sub-9. Kept as the fallback for
//! untagged scenarios and as the temporary stand-in for `@web` (until
//! PR 11 wires `playwright-bdd`) and `@tui` (until expectrl scraping
//! is hardened).

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use tanren_app_services::project::{
    ActiveProjectQuery, ConnectExistingRepositoryCommand, CreateNewProjectCommand,
    ListVisibleProjectsQuery,
};
use tanren_app_services::{Clock, Handlers, Store};
use tanren_contract::{
    AcceptInvitationRequest, ActiveProjectRequest, ActiveProjectView,
    ConnectProjectRepositoryRequest, ConnectProjectRepositoryResponse, CreateProjectRequest,
    CreateProjectResponse, ListVisibleProjectsRequest, ProjectCollectionView, SignInRequest,
    SignUpRequest,
};
use tanren_identity_policy::Argon2idVerifier;
use tanren_provider_integrations::{
    FixtureSourceControlConfig, FixtureSourceControlProvider, SourceControlProvider,
};
use tanren_store::{AccountStore, EventEnvelope, NewInvitation};

use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind, HarnessResult,
    HarnessSession, ProjectHarness,
};

/// In-process harness that drives `tanren_app_services::Handlers`
/// against an ephemeral `SQLite` store. Used for untagged scenarios and
/// as the temporary stand-in for `@web` / `@tui` until those harnesses
/// land their real wire drivers.
pub struct InProcessHarness {
    store: Store,
    handlers: Handlers,
    source_control: Arc<dyn SourceControlProvider>,
    fixture_source_control: FixtureSourceControlProvider,
    kind: HarnessKind,
}

impl std::fmt::Debug for InProcessHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InProcessHarness")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl InProcessHarness {
    /// Construct a fresh in-process harness. Connects an in-memory
    /// `SQLite` store, applies migrations, and drives handlers with a
    /// live clock that calls `Utc::now()` on every invocation.
    ///
    /// A live clock matters because BDD scenarios interleave setup
    /// steps with handler calls that depend on real time progression.
    /// Invitation `expires_at` checks compare to the handler's
    /// `clock.now()`, so a frozen clock captured at construction would
    /// let an invitation registered with a past `expires_at` still
    /// appear unexpired when the scenario runs the acceptance step
    /// (Codex P2 review on PR #133).
    ///
    /// # Errors
    ///
    /// Returns an error if the in-memory store cannot be connected or
    /// migrated.
    pub async fn new(kind: HarnessKind) -> HarnessResult<Self> {
        let store = crate::ephemeral_store()
            .await
            .map_err(|e| HarnessError::Transport(format!("ephemeral store: {e}")))?;
        let clock = Clock::from_fn(Utc::now);
        let handlers = Handlers::with_verifier(clock, Arc::new(Argon2idVerifier::fast_for_tests()));
        let fixture_source_control =
            FixtureSourceControlProvider::new(FixtureSourceControlConfig::default());
        let source_control: Arc<dyn SourceControlProvider> =
            Arc::new(fixture_source_control.clone());
        Ok(Self {
            store,
            handlers,
            source_control,
            fixture_source_control,
            kind,
        })
    }

    /// Borrow the handle of the underlying store. Exposed so the
    /// fallback `@tui` / `@web` paths can read events out alongside
    /// the trait-driven path. Production-shape callers must go
    /// through the [`AccountHarness`] trait.
    #[must_use]
    pub fn store(&self) -> &Store {
        &self.store
    }
}

#[async_trait]
impl AccountHarness for InProcessHarness {
    fn kind(&self) -> HarnessKind {
        self.kind
    }

    async fn sign_up(&mut self, req: SignUpRequest) -> HarnessResult<HarnessSession> {
        match self.handlers.sign_up(&self.store, req).await {
            Ok(response) => Ok(HarnessSession {
                account: response.account.clone(),
                account_id: response.account.id,
                expires_at: response.session.expires_at,
                has_token: !response.session.token.expose_secret().is_empty(),
            }),
            Err(err) => Err(translate_app_error(err)),
        }
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        match self.handlers.sign_in(&self.store, req).await {
            Ok(response) => Ok(HarnessSession {
                account: response.account.clone(),
                account_id: response.account.id,
                expires_at: response.session.expires_at,
                has_token: !response.session.token.expose_secret().is_empty(),
            }),
            Err(err) => Err(translate_app_error(err)),
        }
    }

    async fn accept_invitation(
        &mut self,
        req: AcceptInvitationRequest,
    ) -> HarnessResult<HarnessAcceptance> {
        match self.handlers.accept_invitation(&self.store, req).await {
            Ok(response) => Ok(HarnessAcceptance {
                session: HarnessSession {
                    account: response.account.clone(),
                    account_id: response.account.id,
                    expires_at: response.session.expires_at,
                    has_token: !response.session.token.expose_secret().is_empty(),
                },
                joined_org: response.joined_org,
            }),
            Err(err) => Err(translate_app_error(err)),
        }
    }

    async fn seed_invitation(&mut self, fixture: HarnessInvitation) -> HarnessResult<()> {
        self.store
            .seed_invitation(NewInvitation {
                token: fixture.token,
                inviting_org_id: fixture.inviting_org,
                expires_at: fixture.expires_at,
            })
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_invitation: {e}")))?;
        Ok(())
    }

    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<EventEnvelope>> {
        AccountStore::recent_events(&self.store, limit)
            .await
            .map_err(|e| HarnessError::Transport(format!("recent_events: {e}")))
    }
}

#[async_trait]
impl ProjectHarness for InProcessHarness {
    async fn connect_project_repository(
        &mut self,
        req: ConnectProjectRepositoryRequest,
    ) -> HarnessResult<ConnectProjectRepositoryResponse> {
        self.connect_project_repository_as_actor(req.owning_account_id, req)
            .await
    }

    async fn connect_project_repository_as_actor(
        &mut self,
        actor_account_id: tanren_identity_policy::AccountId,
        req: ConnectProjectRepositoryRequest,
    ) -> HarnessResult<ConnectProjectRepositoryResponse> {
        self.handlers
            .connect_project_repository(
                &self.store,
                self.source_control.as_ref(),
                ConnectExistingRepositoryCommand {
                    actor_account_id,
                    request: req,
                },
            )
            .await
            .map_err(translate_app_error)
    }

    async fn list_visible_projects(
        &mut self,
        req: ListVisibleProjectsRequest,
    ) -> HarnessResult<ProjectCollectionView> {
        self.list_visible_projects_as_actor(req.owning_account_id, req)
            .await
    }

    async fn list_visible_projects_as_actor(
        &mut self,
        actor_account_id: tanren_identity_policy::AccountId,
        req: ListVisibleProjectsRequest,
    ) -> HarnessResult<ProjectCollectionView> {
        self.handlers
            .list_visible_projects(
                &self.store,
                ListVisibleProjectsQuery {
                    actor_account_id,
                    request: req,
                },
            )
            .await
            .map_err(translate_app_error)
    }

    async fn create_project(
        &mut self,
        req: CreateProjectRequest,
    ) -> HarnessResult<CreateProjectResponse> {
        self.create_project_as_actor(req.owning_account_id, req)
            .await
    }

    async fn create_project_as_actor(
        &mut self,
        actor_account_id: tanren_identity_policy::AccountId,
        req: CreateProjectRequest,
    ) -> HarnessResult<CreateProjectResponse> {
        self.handlers
            .create_project(
                &self.store,
                self.source_control.as_ref(),
                CreateNewProjectCommand {
                    actor_account_id,
                    request: req,
                },
            )
            .await
            .map_err(translate_app_error)
    }

    async fn active_project(
        &mut self,
        req: ActiveProjectRequest,
    ) -> HarnessResult<ActiveProjectView> {
        self.active_project_as_actor(req.owning_account_id, req)
            .await
    }

    async fn active_project_as_actor(
        &mut self,
        actor_account_id: tanren_identity_policy::AccountId,
        req: ActiveProjectRequest,
    ) -> HarnessResult<ActiveProjectView> {
        self.handlers
            .active_project(
                &self.store,
                ActiveProjectQuery {
                    actor_account_id,
                    request: req,
                },
            )
            .await
            .map_err(translate_app_error)
    }

    async fn set_repository_access(
        &mut self,
        actor_account_id: tanren_identity_policy::AccountId,
        repository: tanren_identity_policy::RepositoryRef,
        allowed: bool,
    ) -> HarnessResult<()> {
        self.fixture_source_control
            .set_repository_access(actor_account_id, repository, allowed);
        Ok(())
    }

    async fn set_designated_host_create_access(
        &mut self,
        actor_account_id: tanren_identity_policy::AccountId,
        host: tanren_identity_policy::DesignatedHost,
        allowed: bool,
    ) -> HarnessResult<()> {
        self.fixture_source_control.set_host_reachable(&host, true);
        self.fixture_source_control
            .set_host_create_access(actor_account_id, &host, allowed);
        Ok(())
    }

    async fn repository_created_at_host(
        &self,
        host: &tanren_identity_policy::DesignatedHost,
        repository: &tanren_identity_policy::RepositoryRef,
    ) -> HarnessResult<bool> {
        Ok(self
            .fixture_source_control
            .repository_created_at_host(host, repository))
    }

    async fn source_control_call_counters(
        &mut self,
    ) -> HarnessResult<tanren_provider_integrations::SourceControlCallCounters> {
        Ok(self.fixture_source_control.call_counters())
    }
}

fn translate_app_error(err: tanren_app_services::AppServiceError) -> HarnessError {
    use tanren_app_services::AppServiceError;
    match err {
        AppServiceError::Account(reason) => HarnessError::Account(reason, reason.code().to_owned()),
        AppServiceError::Project(reason) => HarnessError::Project(reason, reason.code().to_owned()),
        AppServiceError::InvalidInput(msg) => {
            HarnessError::Transport(format!("invalid_input: {msg}"))
        }
        AppServiceError::Store(err) => HarnessError::Transport(format!("store: {err}")),
        _ => HarnessError::Transport("unknown app-service failure".to_owned()),
    }
}
