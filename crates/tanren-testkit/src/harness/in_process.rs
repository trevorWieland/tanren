//! Direct-`Handlers` harness — the legacy path the rest of the test
//! suite ran on before R-0001 sub-9. Kept as the fallback for
//! untagged scenarios and as the temporary stand-in for `@web` (until
//! PR 11 wires `playwright-bdd`) and `@tui` (until expectrl scraping
//! is hardened).

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use tanren_app_services::{Clock, Handlers, Store};
use tanren_contract::{
    AcceptInvitationRequest, CreateOrganizationRequest, OrganizationAdminOperation,
    OrganizationFailureReason, SignInRequest, SignUpRequest,
};
use tanren_identity_policy::{AccountId, Argon2idVerifier, OrgId, OrgPermission};
use tanren_store::{AccountStore, EventEnvelope, NewInvitation};

use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind,
    HarnessOrgSummary, HarnessOrganization, HarnessResult, HarnessSession,
};

/// In-process harness that drives `tanren_app_services::Handlers`
/// against an ephemeral `SQLite` store. Used for untagged scenarios and
/// as the temporary stand-in for `@web` / `@tui` until those harnesses
/// land their real wire drivers.
pub struct InProcessHarness {
    store: Store,
    handlers: Handlers,
    kind: HarnessKind,
    current_account_id: Option<AccountId>,
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
            current_account_id: None,
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
            Ok(response) => {
                self.current_account_id = Some(response.account.id);
                Ok(HarnessSession {
                    account: response.account.clone(),
                    account_id: response.account.id,
                    expires_at: response.session.expires_at,
                    has_token: !response.session.token.expose_secret().is_empty(),
                })
            }
            Err(err) => Err(translate_app_error(err)),
        }
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        match self.handlers.sign_in(&self.store, req).await {
            Ok(response) => {
                self.current_account_id = Some(response.account.id);
                Ok(HarnessSession {
                    account: response.account.clone(),
                    account_id: response.account.id,
                    expires_at: response.session.expires_at,
                    has_token: !response.session.token.expose_secret().is_empty(),
                })
            }
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

    async fn create_organization(
        &mut self,
        req: CreateOrganizationRequest,
    ) -> HarnessResult<HarnessOrganization> {
        let account_id = self.current_account_id.ok_or_else(|| {
            HarnessError::Organization(
                OrganizationFailureReason::AuthRequired,
                "no signed-in account".to_owned(),
            )
        })?;
        match self
            .handlers
            .create_organization_for_account(&self.store, account_id, req)
            .await
        {
            Ok(out) => Ok(HarnessOrganization {
                org_id: out.organization.id,
                name: out.organization.name.to_string(),
                granted_permissions: out.granted_permissions,
                project_count: out.project_count,
            }),
            Err(err) => Err(translate_app_error(err)),
        }
    }

    async fn list_available_organizations(&mut self) -> HarnessResult<Vec<HarnessOrgSummary>> {
        let account_id = self.current_account_id.ok_or_else(|| {
            HarnessError::Organization(
                OrganizationFailureReason::AuthRequired,
                "no signed-in account".to_owned(),
            )
        })?;
        let orgs = self
            .handlers
            .list_account_organizations(&self.store, account_id)
            .await
            .map_err(translate_app_error)?;
        Ok(orgs
            .into_iter()
            .map(|v| HarnessOrgSummary {
                id: v.id,
                name: v.name.to_string(),
            })
            .collect())
    }

    async fn authorize_admin_operation(
        &mut self,
        org_id: OrgId,
        operation: OrganizationAdminOperation,
    ) -> HarnessResult<()> {
        let account_id = self.current_account_id.ok_or_else(|| {
            HarnessError::Organization(
                OrganizationFailureReason::AuthRequired,
                "no signed-in account".to_owned(),
            )
        })?;
        self.handlers
            .authorize_org_admin_operation(&self.store, account_id, org_id, operation)
            .await
            .map_err(translate_app_error)
    }

    async fn probe_last_admin_protection(
        &mut self,
        org_id: OrgId,
        permission: OrgPermission,
    ) -> HarnessResult<()> {
        let account_id = self.current_account_id.ok_or_else(|| {
            HarnessError::Organization(
                OrganizationFailureReason::AuthRequired,
                "no signed-in account".to_owned(),
            )
        })?;
        self.handlers
            .assert_not_last_admin_holder(&self.store, org_id, account_id, permission)
            .await
            .map_err(translate_app_error)
    }
}

fn translate_app_error(err: tanren_app_services::AppServiceError) -> HarnessError {
    use tanren_app_services::AppServiceError;
    match err {
        AppServiceError::Account(reason) => HarnessError::Account(reason, reason.code().to_owned()),
        AppServiceError::Organization(reason) => {
            HarnessError::Organization(reason, reason.code().to_owned())
        }
        AppServiceError::InvalidInput(msg) => {
            HarnessError::Transport(format!("invalid_input: {msg}"))
        }
        AppServiceError::Store(err) => HarnessError::Transport(format!("store: {err}")),
        _ => HarnessError::Transport("unknown app-service failure".to_owned()),
    }
}
