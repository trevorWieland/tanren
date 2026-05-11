//! Direct-`Handlers` harness — the legacy path the rest of the test
//! suite ran on before R-0001 sub-9. Kept as the fallback for
//! untagged scenarios and as the temporary stand-in for `@web` (until
//! PR 11 wires `playwright-bdd`).

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use secrecy::SecretString;
use tanren_app_services::{Clock, Handlers, Store};
use tanren_contract::{
    AcceptInvitationRequest, CheckOrganizationPermissionRequest,
    CheckOrganizationPermissionResponse, CreateOrganizationRequest, CreateOrganizationResponse,
    LIST_ORGANIZATION_MEMBERS_DEFAULT_LIMIT, LIST_ORGANIZATIONS_DEFAULT_LIMIT,
    ListOrganizationMembersRequest, ListOrganizationMembersResponse, ListOrganizationsRequest,
    ListOrganizationsResponse, SignInRequest, SignUpRequest,
};
use tanren_identity_policy::{
    AccountId, Argon2idVerifier, OrgId, OrganizationName, OrganizationPermission, SessionToken,
};
use tanren_store::{AccountStore, EventEnvelope, NewInvitation};

use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind, HarnessResult,
    HarnessSession,
};

/// In-process harness that drives `tanren_app_services::Handlers`
/// against an ephemeral `SQLite` store. Used for untagged scenarios and
/// as the temporary stand-in for `@web` until that harness lands its
/// real wire driver.
pub struct InProcessHarness {
    store: Store,
    handlers: Handlers,
    kind: HarnessKind,
    sessions: HashMap<AccountId, SessionToken>,
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
            sessions: HashMap::new(),
        })
    }

    /// Borrow the handle of the underlying store. Exposed so the
    /// fallback `@web` path can read events out alongside
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
                self.sessions
                    .insert(response.account.id, response.session.token.clone());
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
                self.sessions
                    .insert(response.account.id, response.session.token.clone());
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
                self.sessions
                    .insert(response.account.id, response.session.token.clone());
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

    async fn create_organization(
        &mut self,
        account_id: AccountId,
        name: OrganizationName,
    ) -> HarnessResult<CreateOrganizationResponse> {
        let session_token = self
            .sessions
            .get(&account_id)
            .cloned()
            .unwrap_or_else(|| SessionToken::from_secret(SecretString::from("")));
        self.handlers
            .create_organization(
                &self.store,
                CreateOrganizationRequest::from_api(
                    session_token,
                    account_id,
                    tanren_contract::CreateOrganizationApiRequest::new(name, None),
                ),
            )
            .await
            .map_err(translate_app_error)
    }

    async fn list_organizations(
        &mut self,
        account_id: AccountId,
    ) -> HarnessResult<ListOrganizationsResponse> {
        let Some(session_token) = self.sessions.get(&account_id).cloned() else {
            return Err(super::auth_required_failure());
        };
        self.handlers
            .list_organizations(
                &self.store,
                ListOrganizationsRequest::from_api_query(
                    session_token,
                    account_id,
                    &tanren_contract::ListOrganizationsApiQuery {
                        limit: Some(LIST_ORGANIZATIONS_DEFAULT_LIMIT),
                        cursor: None,
                    },
                ),
            )
            .await
            .map_err(translate_app_error)
    }

    async fn list_organization_members(
        &mut self,
        account_id: AccountId,
        org_id: OrgId,
    ) -> HarnessResult<ListOrganizationMembersResponse> {
        let Some(session_token) = self.sessions.get(&account_id).cloned() else {
            return Err(super::auth_required_failure());
        };
        self.handlers
            .list_organization_members(
                &self.store,
                ListOrganizationMembersRequest::from_api_query(
                    session_token,
                    account_id,
                    &tanren_contract::ListOrganizationMembersApiPath { org_id },
                    &tanren_contract::ListOrganizationMembersApiQuery {
                        limit: Some(LIST_ORGANIZATION_MEMBERS_DEFAULT_LIMIT),
                        cursor: None,
                    },
                ),
            )
            .await
            .map_err(translate_app_error)
    }

    async fn check_organization_admin_permission(
        &mut self,
        account_id: AccountId,
        org_id: OrgId,
        permission: OrganizationPermission,
    ) -> HarnessResult<CheckOrganizationPermissionResponse> {
        let Some(session_token) = self.sessions.get(&account_id).cloned() else {
            return Err(super::auth_required_failure());
        };
        match self
            .handlers
            .check_organization_permission(
                &self.store,
                CheckOrganizationPermissionRequest::from_api(
                    session_token,
                    account_id,
                    &tanren_contract::CheckOrganizationPermissionApiRequest::new(
                        org_id, permission,
                    ),
                ),
            )
            .await
        {
            Ok(response) if response.allowed => Ok(response),
            Ok(_) => Err(HarnessError::Account(
                tanren_contract::AccountFailureReason::PermissionDenied,
                tanren_contract::AccountFailureReason::PermissionDenied
                    .summary()
                    .to_owned(),
            )),
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
        AppServiceError::CreateOrganization(reason) => {
            HarnessError::Transport(format!("{}: {}", reason.code(), reason.summary()))
        }
        AppServiceError::InvalidInput(msg) => {
            HarnessError::Transport(format!("invalid_input: {msg}"))
        }
        AppServiceError::Store(err) => HarnessError::Transport(format!("store: {err}")),
        _ => HarnessError::Transport("unknown app-service failure".to_owned()),
    }
}
