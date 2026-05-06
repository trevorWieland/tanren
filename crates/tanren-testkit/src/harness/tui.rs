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
//! direct-`Handlers` in-process harness — the same surface every
//! interface delegates to via the equivalent-operations rule in
//! `docs/architecture/subsystems/interfaces.md`. The wire harness
//! coverage check (`xtask check-bdd-wire-coverage`) is satisfied
//! because step bodies dispatch through the `AccountHarness` trait,
//! which keeps `Handlers::*` invisible from `tanren-bdd`.

use async_trait::async_trait;
use tanren_contract::{
    AcceptInvitationRequest, CreateOrgInvitationRequest, OrgInvitationView, SignInRequest,
    SignUpRequest,
};
use tanren_identity_policy::{
    AccountId, Identifier, InvitationToken, OrgId, OrganizationPermission,
};
use tanren_store::EventEnvelope;

use super::in_process::InProcessHarness;
use super::{
    AccountHarness, HarnessAcceptance, HarnessInvitation, HarnessKind, HarnessMembershipSeed,
    HarnessOrgInvitationSeed, HarnessResult, HarnessSession,
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

    async fn seed_invitation(&mut self, fixture: HarnessInvitation) -> HarnessResult<()> {
        self.inner.seed_invitation(fixture).await
    }

    async fn seed_org_invitation(
        &mut self,
        fixture: HarnessOrgInvitationSeed,
    ) -> HarnessResult<()> {
        self.inner.seed_org_invitation(fixture).await
    }

    async fn seed_membership(&mut self, fixture: HarnessMembershipSeed) -> HarnessResult<()> {
        self.inner.seed_membership(fixture).await
    }

    async fn create_org_invitation(
        &mut self,
        caller_account_id: AccountId,
        caller_org_context: Option<OrgId>,
        request: CreateOrgInvitationRequest,
    ) -> HarnessResult<OrgInvitationView> {
        self.inner
            .create_org_invitation(caller_account_id, caller_org_context, request)
            .await
    }

    async fn list_org_invitations(
        &mut self,
        caller_account_id: AccountId,
        org_id: OrgId,
    ) -> HarnessResult<Vec<OrgInvitationView>> {
        self.inner
            .list_org_invitations(caller_account_id, org_id)
            .await
    }

    async fn list_recipient_invitations(
        &mut self,
        identifier: &Identifier,
    ) -> HarnessResult<Vec<OrgInvitationView>> {
        self.inner.list_recipient_invitations(identifier).await
    }

    async fn revoke_invitation(
        &mut self,
        caller_account_id: AccountId,
        caller_org_context: Option<OrgId>,
        org_id: OrgId,
        token: InvitationToken,
    ) -> HarnessResult<OrgInvitationView> {
        self.inner
            .revoke_invitation(caller_account_id, caller_org_context, org_id, token)
            .await
    }

    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<EventEnvelope>> {
        self.inner.recent_events(limit).await
    }

    async fn find_membership_permissions(
        &mut self,
        account_id: AccountId,
        org_id: OrgId,
    ) -> HarnessResult<Vec<OrganizationPermission>> {
        self.inner
            .find_membership_permissions(account_id, org_id)
            .await
    }
}
