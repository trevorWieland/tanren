//! `@web` fallback harness — delegates to [`super::InProcessHarness`].
//!
//! The real-browser witness for B-0066 runs via `playwright-bdd` in
//! `apps/web/tests/bdd`. The Rust runner intentionally filters those
//! B-0066 `@web` scenarios out so the witness is produced by Chromium.
//! This harness remains available for other `@web` scenarios that still
//! opt into fast in-process feedback.

use async_trait::async_trait;
use tanren_contract::{
    AcceptInvitationRequest, CheckOrganizationPermissionResponse, CreateOrganizationResponse,
    ListOrganizationMembersResponse, ListOrganizationsResponse, SignInRequest, SignUpRequest,
};
use tanren_identity_policy::{AccountId, OrgId, OrganizationName, OrganizationPermission};
use tanren_store::EventEnvelope;

use super::in_process::InProcessHarness;
use super::{
    AccountHarness, HarnessAcceptance, HarnessInvitation, HarnessKind, HarnessResult,
    HarnessSession,
};

/// `@web` fallback wrapper around [`InProcessHarness`]. B-0066 web
/// witness coverage runs in Playwright; this harness keeps non-B-0066
/// Rust BDD scenarios self-contained.
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

    async fn create_organization(
        &mut self,
        account_id: AccountId,
        name: OrganizationName,
    ) -> HarnessResult<CreateOrganizationResponse> {
        self.inner.create_organization(account_id, name).await
    }

    async fn list_organizations(
        &mut self,
        account_id: AccountId,
    ) -> HarnessResult<ListOrganizationsResponse> {
        self.inner.list_organizations(account_id).await
    }

    async fn list_organization_members(
        &mut self,
        account_id: AccountId,
        org_id: OrgId,
    ) -> HarnessResult<ListOrganizationMembersResponse> {
        self.inner
            .list_organization_members(account_id, org_id)
            .await
    }

    async fn check_organization_admin_permission(
        &mut self,
        account_id: AccountId,
        org_id: OrgId,
        permission: OrganizationPermission,
    ) -> HarnessResult<CheckOrganizationPermissionResponse> {
        self.inner
            .check_organization_admin_permission(account_id, org_id, permission)
            .await
    }

    async fn seed_invitation(&mut self, fixture: HarnessInvitation) -> HarnessResult<()> {
        self.inner.seed_invitation(fixture).await
    }

    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<EventEnvelope>> {
        self.inner.recent_events(limit).await
    }
}
