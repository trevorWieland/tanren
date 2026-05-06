//! Direct-`Handlers` harness — the legacy path the rest of the test
//! suite ran on before R-0001 sub-9. Kept as the fallback for
//! untagged scenarios and as the temporary stand-in for `@web` (until
//! PR 11 wires `playwright-bdd`) and `@tui` (until expectrl scraping
//! is hardened).

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use tanren_app_services::{AccountStore, Clock, Handlers, Store};
use tanren_contract::{
    AcceptInvitationRequest, CreateOrgInvitationRequest, RevokeOrgInvitationRequest, SignInRequest,
    SignUpRequest,
};
use tanren_identity_policy::{
    AccountId, Argon2idVerifier, Identifier, InvitationToken, OrgId, OrganizationPermission,
};
use tanren_store::{CreateInvitation, EventEnvelope, NewInvitation};

use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind,
    HarnessMembershipSeed, HarnessOrgInvitationSeed, HarnessResult, HarnessSession,
};

/// In-process harness that drives `tanren_app_services::Handlers`
/// against an ephemeral `SQLite` store. Used for untagged scenarios and
/// as the temporary stand-in for `@web` / `@tui` until those harnesses
/// land their real wire drivers.
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
        Ok(Self {
            store,
            handlers,
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

    async fn seed_org_invitation(
        &mut self,
        fixture: HarnessOrgInvitationSeed,
    ) -> HarnessResult<()> {
        self.store
            .seed_organization_invitation(CreateInvitation {
                token: fixture.token,
                inviting_org_id: fixture.org_id,
                recipient_identifier: fixture.recipient_identifier,
                granted_permissions: fixture.permissions,
                created_by_account_id: fixture.created_by,
                created_at: Utc::now(),
                expires_at: fixture.expires_at,
            })
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_org_invitation: {e}")))?;
        Ok(())
    }

    async fn seed_membership(&mut self, fixture: HarnessMembershipSeed) -> HarnessResult<()> {
        self.store
            .insert_membership(
                fixture.account_id,
                fixture.org_id,
                fixture.permissions,
                Utc::now(),
            )
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_membership: {e}")))?;
        Ok(())
    }

    async fn create_org_invitation(
        &mut self,
        caller_account_id: AccountId,
        caller_org_context: Option<OrgId>,
        request: CreateOrgInvitationRequest,
    ) -> HarnessResult<tanren_contract::OrgInvitationView> {
        match self
            .handlers
            .create_invitation(&self.store, caller_account_id, caller_org_context, request)
            .await
        {
            Ok(response) => Ok(response.invitation),
            Err(err) => Err(translate_app_error(err)),
        }
    }

    async fn list_org_invitations(
        &mut self,
        caller_account_id: AccountId,
        org_id: OrgId,
    ) -> HarnessResult<Vec<tanren_contract::OrgInvitationView>> {
        match self
            .handlers
            .list_org_invitations(&self.store, caller_account_id, org_id)
            .await
        {
            Ok(response) => Ok(response.invitations),
            Err(err) => Err(translate_app_error(err)),
        }
    }

    async fn list_recipient_invitations(
        &mut self,
        identifier: &Identifier,
    ) -> HarnessResult<Vec<tanren_contract::OrgInvitationView>> {
        match self
            .handlers
            .list_recipient_invitations(&self.store, identifier)
            .await
        {
            Ok(response) => Ok(response.invitations),
            Err(err) => Err(translate_app_error(err)),
        }
    }

    async fn revoke_invitation(
        &mut self,
        caller_account_id: AccountId,
        caller_org_context: Option<OrgId>,
        org_id: OrgId,
        token: InvitationToken,
    ) -> HarnessResult<tanren_contract::OrgInvitationView> {
        let request = RevokeOrgInvitationRequest { org_id, token };
        match self
            .handlers
            .revoke_invitation(&self.store, caller_account_id, caller_org_context, request)
            .await
        {
            Ok(response) => Ok(response.invitation),
            Err(err) => Err(translate_app_error(err)),
        }
    }

    async fn find_membership_permissions(
        &mut self,
        account_id: AccountId,
        org_id: OrgId,
    ) -> HarnessResult<Vec<OrganizationPermission>> {
        AccountStore::find_organization_permissions(&self.store, account_id, org_id)
            .await
            .map_err(|e| HarnessError::Transport(format!("find_membership_permissions: {e}")))
    }
}

fn translate_app_error(err: tanren_app_services::AppServiceError) -> HarnessError {
    use tanren_app_services::AppServiceError;
    match err {
        AppServiceError::Account(reason) => HarnessError::Account(reason, reason.code().to_owned()),
        AppServiceError::InvalidInput(msg) => {
            HarnessError::Transport(format!("invalid_input: {msg}"))
        }
        AppServiceError::Store(err) => HarnessError::Transport(format!("store: {err}")),
        _ => HarnessError::Transport("unknown app-service failure".to_owned()),
    }
}
