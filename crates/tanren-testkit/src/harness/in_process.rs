//! Direct-`Handlers` harness — the legacy path the rest of the test
//! suite ran on before R-0001 sub-9. Kept as the fallback for
//! untagged scenarios and as the temporary stand-in for `@web` (until
//! PR 11 wires `playwright-bdd`). `@tui` now drives the real binary
//! via `expectrl` (see `tui.rs`).

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use tanren_app_services::{Clock, Handlers, Store};
use tanren_contract::{
    AcceptInvitationRequest, AttentionSpecView, ProjectScopedViews, ProjectView, SignInRequest,
    SignUpRequest, SwitchProjectResponse,
};
use tanren_identity_policy::{AccountId, Argon2idVerifier, ProjectId, SpecId};
use tanren_store::{AccountStore, EventEnvelope, NewInvitation, NewProject, NewSpec, ProjectStore};

use super::project::{HarnessProjectFixture, HarnessSpecFixture, ProjectHarness};
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind, HarnessResult,
    HarnessSession,
};

pub struct InProcessHarness {
    store: Store,
    handlers: Handlers,
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
    pub async fn new(kind: HarnessKind) -> HarnessResult<Self> {
        let store = crate::ephemeral_store()
            .await
            .map_err(|e| HarnessError::Transport(format!("ephemeral store: {e}")))?;
        let clock = Clock::from_fn(Utc::now);
        let handlers = Handlers::with_verifier(clock, Arc::new(Argon2idVerifier::fast_for_tests()));
        Ok(Self {
            store,
            handlers,
            kind,
        })
    }

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

#[async_trait]
impl ProjectHarness for InProcessHarness {
    fn kind(&self) -> HarnessKind {
        self.kind
    }

    async fn seed_project(&mut self, fixture: HarnessProjectFixture) -> HarnessResult<ProjectId> {
        let id = fixture.id;
        self.store
            .seed_project(NewProject {
                id,
                account_id: fixture.account_id,
                name: fixture.name,
                state: fixture.state,
                created_at: fixture.created_at,
            })
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_project: {e}")))?;
        Ok(id)
    }

    async fn seed_spec(&mut self, fixture: HarnessSpecFixture) -> HarnessResult<SpecId> {
        let id = fixture.id;
        self.store
            .seed_spec(NewSpec {
                id,
                project_id: fixture.project_id,
                name: fixture.name,
                needs_attention: fixture.needs_attention,
                attention_reason: fixture.attention_reason,
                created_at: fixture.created_at,
            })
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_spec: {e}")))?;
        Ok(id)
    }

    async fn seed_view_state(
        &mut self,
        account_id: AccountId,
        project_id: ProjectId,
        state: serde_json::Value,
    ) -> HarnessResult<()> {
        self.store
            .write_view_state(account_id, project_id, state, Utc::now())
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_view_state: {e}")))?;
        Ok(())
    }

    async fn list_projects(&mut self, account_id: AccountId) -> HarnessResult<Vec<ProjectView>> {
        self.handlers
            .list_projects(&self.store, account_id)
            .await
            .map_err(translate_app_error)
    }

    async fn switch_active_project(
        &mut self,
        account_id: AccountId,
        project_id: ProjectId,
    ) -> HarnessResult<SwitchProjectResponse> {
        self.handlers
            .switch_active_project(&self.store, account_id, project_id)
            .await
            .map_err(translate_app_error)
    }

    async fn attention_spec(
        &mut self,
        account_id: AccountId,
        project_id: ProjectId,
        spec_id: SpecId,
    ) -> HarnessResult<AttentionSpecView> {
        self.handlers
            .attention_spec(&self.store, account_id, project_id, spec_id)
            .await
            .map_err(translate_app_error)
    }

    async fn project_scoped_views(
        &mut self,
        account_id: AccountId,
    ) -> HarnessResult<ProjectScopedViews> {
        let resp = self
            .handlers
            .project_scoped_views(&self.store, account_id)
            .await
            .map_err(translate_app_error)?;
        Ok(ProjectScopedViews {
            project_id: resp.project_id,
            specs: resp.specs,
            loops: resp.loops,
            milestones: resp.milestones,
        })
    }
}
