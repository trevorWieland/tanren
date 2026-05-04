//! `@tui` harness — drives the `tanren-tui` screen state machine via
//! [`tanren_tui_app::harness::TuiDriver`] for organization flows while
//! delegating account operations to [`InProcessHarness`].
//!
//! The `TuiDriver` wraps the same `App` the real binary uses, but
//! bypasses the terminal: keypresses are injected directly and the
//! screen state is read back without rendering. Because the `App`
//! internally calls `runtime.block_on()` to bridge its sync event loop
//! to async handlers, the driver **must** live on its own OS thread —
//! calling `block_on` from inside a tokio async context panics.
//!
//! Account sign-up / sign-in / invitation-acceptance still go through
//! the in-process harness (those screens are covered by B-0043); the
//! TUI-specific org-create/org-list paths are the new coverage added
//! by R-0002.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};

use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use secrecy::SecretString;
use tanren_app_services::{Clock, Handlers, Store};
use tanren_contract::{
    AcceptInvitationRequest, CreateOrganizationRequest, OrganizationFailureReason,
    OrganizationView, SignInRequest, SignUpRequest,
};
use tanren_identity_policy::{Argon2idVerifier, OrgAdminPermissions, SessionToken};
use tanren_store::EventEnvelope;
use tanren_tui_app::harness::{ScreenKind, ScreenSnapshot, TuiDriver};

use super::in_process::InProcessHarness;
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind,
    HarnessOrgCreation, HarnessResult, HarnessSession,
};

fn tui_db_path() -> PathBuf {
    std::env::temp_dir().join(format!("tanren-tui-{}.db", uuid::Uuid::new_v4().simple()))
}

enum TuiCmd {
    SetSessionToken(SessionToken),
    NavigateToDashboard,
    PushKey(KeyEvent),
    GetScreen,
    GetOrgListData,
}

enum TuiReply {
    Screen(ScreenSnapshot),
    OrgListData(Vec<OrganizationView>),
}

struct TuiThread {
    cmd_tx: Sender<TuiCmd>,
    reply_rx: std::sync::Mutex<Receiver<TuiReply>>,
}

impl TuiThread {
    fn spawn(store: Arc<Store>) -> Self {
        let (cmd_tx, cmd_rx): (Sender<TuiCmd>, Receiver<TuiCmd>) = mpsc::channel();
        let (reply_tx, reply_rx): (Sender<TuiReply>, Receiver<TuiReply>) = mpsc::channel();

        std::thread::spawn(move || {
            let mut driver =
                TuiDriver::with_store(store).expect("TuiDriver::with_store must succeed");
            for cmd in &cmd_rx {
                match cmd {
                    TuiCmd::SetSessionToken(token) => {
                        driver.set_session_token(token);
                        let _ = reply_tx.send(TuiReply::Screen(driver.screen()));
                    }
                    TuiCmd::NavigateToDashboard => {
                        driver.navigate_to_dashboard();
                        let _ = reply_tx.send(TuiReply::Screen(driver.screen()));
                    }
                    TuiCmd::PushKey(key) => {
                        driver.push_key(key);
                        let _ = reply_tx.send(TuiReply::Screen(driver.screen()));
                    }
                    TuiCmd::GetScreen => {
                        let _ = reply_tx.send(TuiReply::Screen(driver.screen()));
                    }
                    TuiCmd::GetOrgListData => {
                        let _ = reply_tx.send(TuiReply::OrgListData(driver.org_list_data()));
                    }
                }
            }
        });

        Self {
            cmd_tx,
            reply_rx: std::sync::Mutex::new(reply_rx),
        }
    }

    fn set_token(&self, token: SessionToken) -> ScreenSnapshot {
        self.cmd_tx
            .send(TuiCmd::SetSessionToken(token))
            .expect("tui thread alive");
        self.recv_screen()
    }

    fn navigate_to_dashboard(&self) -> ScreenSnapshot {
        self.cmd_tx
            .send(TuiCmd::NavigateToDashboard)
            .expect("tui thread alive");
        self.recv_screen()
    }

    fn push_key(&self, key: KeyEvent) -> ScreenSnapshot {
        self.cmd_tx
            .send(TuiCmd::PushKey(key))
            .expect("tui thread alive");
        self.recv_screen()
    }

    fn screen(&self) -> ScreenSnapshot {
        self.cmd_tx
            .send(TuiCmd::GetScreen)
            .expect("tui thread alive");
        self.recv_screen()
    }

    fn org_list_data(&self) -> Vec<OrganizationView> {
        self.cmd_tx
            .send(TuiCmd::GetOrgListData)
            .expect("tui thread alive");
        let reply = self
            .reply_rx
            .lock()
            .expect("lock")
            .recv()
            .expect("tui thread alive");
        match reply {
            TuiReply::OrgListData(v) => v,
            TuiReply::Screen(_) => unreachable!("unexpected reply for GetOrgListData"),
        }
    }

    fn recv_screen(&self) -> ScreenSnapshot {
        let reply = self
            .reply_rx
            .lock()
            .expect("lock")
            .recv()
            .expect("tui thread alive");
        match reply {
            TuiReply::Screen(s) => s,
            TuiReply::OrgListData(_) => unreachable!("unexpected reply for screen command"),
        }
    }
}

pub struct TuiHarness {
    inner: InProcessHarness,
    tui_thread: Option<TuiThread>,
    tui_store: Option<Arc<Store>>,
    pending_token: Option<SecretString>,
}

impl std::fmt::Debug for TuiHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TuiHarness")
            .field("kind", &self.kind())
            .finish_non_exhaustive()
    }
}

impl TuiHarness {
    pub async fn spawn() -> HarnessResult<Self> {
        let db_path = tui_db_path();
        let database_url = format!("sqlite:{}?mode=rwc", db_path.display());
        let store = Store::connect(&database_url)
            .await
            .map_err(|e| HarnessError::Transport(format!("connect store: {e}")))?;
        store
            .migrate()
            .await
            .map_err(|e| HarnessError::Transport(format!("migrate store: {e}")))?;
        let store = Arc::new(store);

        let tui_database_url = format!("sqlite:{}", db_path.display());
        let tui_store = Arc::new(
            Store::connect(&tui_database_url)
                .await
                .map_err(|e| HarnessError::Transport(format!("connect tui store: {e}")))?,
        );

        let clock = Clock::from_fn(chrono::Utc::now);
        let handlers = Handlers::with_verifier(clock, Arc::new(Argon2idVerifier::fast_for_tests()));
        let inner = InProcessHarness::from_parts(store, handlers, HarnessKind::Tui);
        Ok(Self {
            inner,
            tui_thread: None,
            tui_store: Some(tui_store),
            pending_token: None,
        })
    }

    fn ensure_tui_thread(&mut self) -> &TuiThread {
        if self.tui_thread.is_none() {
            let store = self.tui_store.take().expect("tui_store present");
            let thread = TuiThread::spawn(store);
            if let Some(secret) = self.pending_token.take() {
                let token = SessionToken::from_secret(secret);
                thread.set_token(token);
            }
            self.tui_thread = Some(thread);
        }
        self.tui_thread.as_ref().expect("just set")
    }

    async fn install_token_via_sign_in(
        &mut self,
        email: tanren_identity_policy::Email,
        password: SecretString,
    ) {
        let sign_in_req = SignInRequest { email, password };
        let store = self.inner.store();
        let handlers = self.inner.handlers();
        if let Ok(response) = handlers.sign_in(store, sign_in_req).await {
            let token = response.session.token;
            self.pending_token = Some(SecretString::from(token.expose_secret().to_owned()));
            if let Some(thread) = self.tui_thread.as_ref() {
                thread.set_token(token);
            }
        }
    }

    fn parse_banner_code(banner: &str) -> OrganizationFailureReason {
        let lower = banner.to_lowercase();
        if lower.contains("unauthenticated") || lower.contains("not authenticated") {
            OrganizationFailureReason::Unauthenticated
        } else if lower.contains("duplicate_name") {
            OrganizationFailureReason::DuplicateName
        } else if lower.contains("not_authorized") {
            OrganizationFailureReason::NotAuthorized
        } else {
            OrganizationFailureReason::Unauthenticated
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
        let name = request.name.as_str().to_owned();
        let thread = self.ensure_tui_thread();
        thread.navigate_to_dashboard();
        thread.push_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let nav_snapshot = thread.screen();
        if nav_snapshot.kind != ScreenKind::OrgCreate {
            return Err(HarnessError::Organization(
                OrganizationFailureReason::Unauthenticated,
                "TUI prevented access to org creation screen".to_owned(),
            ));
        }
        for c in name.chars() {
            thread.push_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        thread.push_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let snapshot = thread.screen();

        match snapshot.kind {
            ScreenKind::OrgList => {
                let orgs = thread.org_list_data();
                let found = orgs
                    .iter()
                    .find(|o| o.name.as_str().eq_ignore_ascii_case(&name))
                    .cloned();
                match found {
                    Some(organization) => {
                        let permissions = self
                            .inner
                            .admin_permissions_for_org(organization.id)
                            .await?;
                        Ok(HarnessOrgCreation {
                            organization,
                            permissions,
                        })
                    }
                    None => Err(HarnessError::Transport(format!(
                        "org '{name}' not found in TUI org list after creation"
                    ))),
                }
            }
            ScreenKind::OrgCreate => {
                let reason = snapshot.banner.as_deref().map_or(
                    OrganizationFailureReason::Unauthenticated,
                    Self::parse_banner_code,
                );
                Err(HarnessError::Organization(
                    reason,
                    snapshot
                        .banner
                        .unwrap_or_else(|| "unknown TUI error".to_owned()),
                ))
            }
            other => Err(HarnessError::Transport(format!(
                "unexpected TUI screen after org create: {other:?}"
            ))),
        }
    }

    async fn list_organizations(&mut self) -> HarnessResult<Vec<OrganizationView>> {
        let thread = self.ensure_tui_thread();
        thread.navigate_to_dashboard();
        thread.push_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        thread.push_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let snapshot = thread.screen();

        if snapshot.kind == ScreenKind::OrgList {
            if let Some(banner) = snapshot.banner {
                let reason = Self::parse_banner_code(&banner);
                return Err(HarnessError::Organization(reason, banner));
            }
            Ok(thread.org_list_data())
        } else {
            Err(HarnessError::Transport(format!(
                "unexpected TUI screen after org list: {:?}",
                snapshot.kind
            )))
        }
    }

    async fn admin_permissions_for_org(
        &mut self,
        org_id: tanren_identity_policy::OrgId,
    ) -> HarnessResult<OrgAdminPermissions> {
        self.inner.admin_permissions_for_org(org_id).await
    }
}
