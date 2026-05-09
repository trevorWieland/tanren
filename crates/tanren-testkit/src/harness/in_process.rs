//! Direct-`Handlers` harness — the legacy path the rest of the test
//! suite ran on before R-0001 sub-9. Kept as the fallback for
//! untagged scenarios and as the temporary stand-in for `@web` (until
//! PR 11 wires `playwright-bdd`) and `@tui` (until expectrl scraping
//! is hardened).

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use tanren_app_services::{Clock, Handlers, MyPermissionsContext, Store};
use tanren_contract::{
    AcceptInvitationRequest, MyPermissionsRequest, SignInRequest, SignUpRequest,
};
use tanren_identity_policy::AccountId;
use tanren_identity_policy::Argon2idVerifier;
use tanren_store::{
    AccountStore, EventEnvelope, NewInvitation, NewPermissionConstraint, NewPermissionGrant,
    PermissionGrantScope,
};

use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind,
    HarnessPermissionGrantFixture, HarnessPermissionScope, HarnessPermissionsView, HarnessResult,
    HarnessSession,
};

/// In-process harness that drives `tanren_app_services::Handlers`
/// against an ephemeral `SQLite` store. Used for untagged scenarios and
/// as the temporary stand-in for `@web` / `@tui` until those harnesses
/// land their real wire drivers.
pub struct InProcessHarness {
    store: Store,
    handlers: Handlers,
    kind: HarnessKind,
    authenticated_accounts: HashSet<AccountId>,
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
            authenticated_accounts: HashSet::new(),
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
                self.authenticated_accounts.insert(response.account.id);
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
                self.authenticated_accounts.insert(response.account.id);
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
            Ok(response) => {
                self.authenticated_accounts.insert(response.account.id);
                Ok(HarnessAcceptance {
                    session: HarnessSession {
                        account: response.account.clone(),
                        account_id: response.account.id,
                        expires_at: response.session.expires_at,
                        has_token: !response.session.token.expose_secret().is_empty(),
                    },
                    joined_org: response.joined_org,
                })
            }
            Err(err) => Err(translate_app_error(err)),
        }
    }

    async fn my_permissions(
        &mut self,
        session_account_id: AccountId,
        requested_account_id: Option<AccountId>,
    ) -> HarnessResult<HarnessPermissionsView> {
        if !self.authenticated_accounts.contains(&session_account_id) {
            return Err(HarnessError::FailureCode {
                code: "auth_required".to_owned(),
                summary: "No authenticated session is present. Sign in and retry.".to_owned(),
            });
        }
        let context = MyPermissionsContext::with_requested_account(
            session_account_id,
            requested_account_id.unwrap_or(session_account_id),
        );
        match self
            .handlers
            .my_permissions(&self.store, context, MyPermissionsRequest::default())
            .await
        {
            Ok(response) => Ok(HarnessPermissionsView {
                rendered: serde_json::to_string(&response).unwrap_or_default(),
                response,
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

    async fn seed_permission_grant(
        &mut self,
        fixture: HarnessPermissionGrantFixture,
    ) -> HarnessResult<()> {
        let scope = match fixture.scope {
            HarnessPermissionScope::Organization(org_id) => {
                PermissionGrantScope::Organization(org_id)
            }
            HarnessPermissionScope::Project(project_id) => {
                PermissionGrantScope::Project(project_id)
            }
        };
        let grant = self
            .store
            .seed_permission_grant(NewPermissionGrant {
                account_id: fixture.account_id,
                scope,
                permission: fixture.permission,
                grant_source: fixture.grant_source,
                created_at: Utc::now(),
            })
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_permission_grant: {e}")))?;
        if let Some(constraint) = fixture.policy_constraint {
            self.store
                .seed_permission_constraint(NewPermissionConstraint {
                    grant_id: grant.id,
                    reason: constraint.reason,
                    source: constraint.source,
                    created_at: Utc::now(),
                })
                .await
                .map_err(|e| HarnessError::Transport(format!("seed_permission_constraint: {e}")))?;
        }
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
        AppServiceError::InvalidInput(msg) => {
            HarnessError::Transport(format!("invalid_input: {msg}"))
        }
        AppServiceError::Permissions(reason) => HarnessError::FailureCode {
            code: reason.code().to_owned(),
            summary: reason.summary().to_owned(),
        },
        AppServiceError::Store(err) => HarnessError::Transport(format!("store: {err}")),
        _ => HarnessError::Transport("unknown app-service failure".to_owned()),
    }
}
