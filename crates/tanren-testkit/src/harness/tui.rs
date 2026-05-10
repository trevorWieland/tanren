//! `@tui` harness — uses the in-process service harness for stateful
//! account actions, and renders self-permissions through the real
//! `tanren-tui-app` ratatui draw path for empirical visibility checks.
//!
//! TODO(R-0001 sub-11 or follow-up): replace the remaining service
//! actions with a full `expectrl` + `portable-pty` driver against the
//! `tanren-tui` binary.

use async_trait::async_trait;
use tanren_contract::{AcceptInvitationRequest, SignInRequest, SignUpRequest};
use tanren_store::EventEnvelope;

use super::in_process::InProcessHarness;
use super::{
    AccountHarness, HarnessAcceptance, HarnessInvitation, HarnessKind, HarnessMyPermissionsQuery,
    HarnessPermissionGrantFixture, HarnessPermissionsCapabilityView, HarnessPermissionsView,
    HarnessResult, HarnessSession,
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

    async fn my_permissions(
        &mut self,
        session_account_id: tanren_identity_policy::AccountId,
        requested_account_id: Option<tanren_identity_policy::AccountId>,
    ) -> HarnessResult<HarnessPermissionsView> {
        self.my_permissions_query(
            session_account_id,
            requested_account_id,
            HarnessMyPermissionsQuery::default(),
        )
        .await
    }

    async fn my_permissions_query(
        &mut self,
        session_account_id: tanren_identity_policy::AccountId,
        requested_account_id: Option<tanren_identity_policy::AccountId>,
        query: HarnessMyPermissionsQuery,
    ) -> HarnessResult<HarnessPermissionsView> {
        let view = self
            .inner
            .my_permissions_query(session_account_id, requested_account_id, query)
            .await?;
        Ok(HarnessPermissionsView {
            rendered: tanren_tui_app::render_my_permissions_screen(&view.response),
            response: view.response,
        })
    }

    async fn my_permissions_capability(
        &mut self,
        session_account_id: tanren_identity_policy::AccountId,
        requested_account_id: Option<tanren_identity_policy::AccountId>,
    ) -> HarnessResult<HarnessPermissionsCapabilityView> {
        self.inner
            .my_permissions_capability(session_account_id, requested_account_id)
            .await
    }

    async fn seed_invitation(&mut self, fixture: HarnessInvitation) -> HarnessResult<()> {
        self.inner.seed_invitation(fixture).await
    }

    async fn seed_permission_grant(
        &mut self,
        fixture: HarnessPermissionGrantFixture,
    ) -> HarnessResult<()> {
        self.inner.seed_permission_grant(fixture).await
    }

    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<EventEnvelope>> {
        self.inner.recent_events(limit).await
    }
}
