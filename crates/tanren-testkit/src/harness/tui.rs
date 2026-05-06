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
//! `TuiHarness` wrapper rather than through the generic
//! `ProjectInProcessHarness` facade. Step bodies dispatch through the
//! `AccountHarness` / `ProjectHarness` traits, which keeps
//! `Handlers::*` invisible from `tanren-bdd`.

use async_trait::async_trait;
use tanren_app_services::{Handlers, Store};
use tanren_contract::{AcceptInvitationRequest, SignInRequest, SignUpRequest};
use tanren_store::EventEnvelope;

use super::in_process::InProcessHarness;
use super::{
    AccountHarness, HarnessAcceptance, HarnessInvitation, HarnessKind, HarnessResult,
    HarnessSession,
};

/// `@tui` harness — wrapper around [`InProcessHarness`] until the
/// expectrl-based driver lands. Exposes the underlying store and
/// handlers so that `ProjectTuiHarness` can drive project operations
/// through the same per-surface seam.
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

    #[must_use]
    pub(crate) fn store_handle(&self) -> &Store {
        self.inner.store()
    }

    pub(crate) fn store_handle_mut(&mut self) -> &mut Store {
        self.inner.store_mut()
    }

    #[must_use]
    pub(crate) fn handlers(&self) -> &Handlers {
        self.inner.handlers()
    }

    #[must_use]
    pub(crate) fn provider_connection_id(&self) -> tanren_identity_policy::ProviderConnectionId {
        self.inner.provider_connection_id()
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
}
