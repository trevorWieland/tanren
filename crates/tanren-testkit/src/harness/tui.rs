//! `@tui` harness — drives the `tanren-tui` screen state machine via
//! [`tanren_tui_app::harness::TuiDriver`] for organization flows while
//! delegating account operations to [`InProcessHarness`].
//!
//! The `TuiDriver` wraps the same `App` the real binary uses, but
//! bypasses the terminal: keypresses are injected directly and the
//! screen state is read back without rendering. Account sign-up /
//! sign-in / invitation-acceptance still go through the in-process
//! harness (those screens are covered by B-0043); the TUI-specific
//! org-create/org-list paths are the new coverage added by R-0002.

use std::sync::Arc;

use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tanren_contract::{
    AcceptInvitationRequest, CreateOrganizationRequest, OrganizationView, SignInRequest,
    SignUpRequest,
};
use tanren_identity_policy::OrgAdminPermissions;
use tanren_store::EventEnvelope;
use tanren_tui_app::harness::{ScreenSnapshot, TuiDriver};

use super::in_process::InProcessHarness;
use super::{
    AccountHarness, HarnessAcceptance, HarnessInvitation, HarnessKind, HarnessOrgCreation,
    HarnessResult, HarnessSession,
};

pub struct TuiHarness {
    inner: InProcessHarness,
    driver: Option<TuiDriver>,
}

impl std::fmt::Debug for TuiHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TuiHarness")
            .field("kind", &self.kind())
            .finish_non_exhaustive()
    }
}

impl TuiHarness {
    /// Construct the TUI harness. Creates a shared in-process store,
    /// then initialises a [`TuiDriver`] backed by the same store so
    /// org operations the driver submits are visible to the inner
    /// harness and vice-versa.
    ///
    /// # Errors
    ///
    /// Returns an error if the ephemeral store or TUI driver cannot
    /// be initialised.
    pub async fn spawn() -> HarnessResult<Self> {
        let inner = InProcessHarness::new(HarnessKind::Tui).await?;
        Ok(Self {
            inner,
            driver: None,
        })
    }

    fn ensure_driver(&mut self) -> &mut TuiDriver {
        if self.driver.is_none() {
            let store = Arc::new(self.inner.store().clone());
            self.driver =
                Some(TuiDriver::with_store(store).expect("TuiDriver::with_store must succeed"));
        }
        self.driver.as_mut().expect("just set")
    }

    /// Drive the TUI through the org-create flow: navigate to
    /// Dashboard, select "Create organization", type the name
    /// character-by-character, and submit.
    ///
    /// The driver must already hold a session token (set during a
    /// prior `sign_up` or `sign_in` call).
    ///
    /// Returns the resulting [`ScreenSnapshot`] — on success the
    /// `kind` field is `OrgList`; on rejection the banner contains a
    /// `code:` token.
    pub fn create_organization(&mut self, name: &str) -> ScreenSnapshot {
        let driver = self.ensure_driver();
        driver.navigate_to_dashboard();
        driver.push_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        for c in name.chars() {
            driver.push_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        driver.push_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        driver.screen()
    }

    /// Drive the TUI to the org-list screen from the Dashboard.
    ///
    /// The driver must already hold a session token.
    pub fn list_organizations(&mut self) -> ScreenSnapshot {
        let driver = self.ensure_driver();
        driver.navigate_to_dashboard();
        driver.push_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        driver.push_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        driver.screen()
    }

    async fn install_token_via_sign_in(
        &mut self,
        email: tanren_identity_policy::Email,
        password: secrecy::SecretString,
    ) {
        let sign_in_req = SignInRequest { email, password };
        let store = self.inner.store();
        let handlers = self.inner.handlers();
        if let Ok(response) = handlers.sign_in(store, sign_in_req).await {
            if let Some(driver) = self.driver.as_mut() {
                driver.set_session_token(response.session.token);
            }
        }
    }
}

#[async_trait]
impl AccountHarness for TuiHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Tui
    }

    async fn sign_up(&mut self, req: SignUpRequest) -> HarnessResult<HarnessSession> {
        let email = req.email.clone();
        let password = req.password.clone();
        let session = self.inner.sign_up(req).await?;
        self.install_token_via_sign_in(email, password).await;
        Ok(session)
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        let email = req.email.clone();
        let password = req.password.clone();
        let session = self.inner.sign_in(req).await?;
        self.install_token_via_sign_in(email, password).await;
        Ok(session)
    }

    async fn accept_invitation(
        &mut self,
        req: AcceptInvitationRequest,
    ) -> HarnessResult<HarnessAcceptance> {
        let email = req.email.clone();
        let password = req.password.clone();
        let acceptance = self.inner.accept_invitation(req).await?;
        self.install_token_via_sign_in(email, password).await;
        Ok(acceptance)
    }

    async fn seed_invitation(&mut self, fixture: HarnessInvitation) -> HarnessResult<()> {
        self.inner.seed_invitation(fixture).await
    }

    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<EventEnvelope>> {
        self.inner.recent_events(limit).await
    }

    async fn create_organization(
        &mut self,
        request: CreateOrganizationRequest,
    ) -> HarnessResult<HarnessOrgCreation> {
        self.inner.create_organization(request).await
    }

    async fn list_organizations(&mut self) -> HarnessResult<Vec<OrganizationView>> {
        self.inner.list_organizations().await
    }

    async fn admin_permissions_for_org(
        &mut self,
        org_id: tanren_identity_policy::OrgId,
    ) -> HarnessResult<OrgAdminPermissions> {
        self.inner.admin_permissions_for_org(org_id).await
    }
}
