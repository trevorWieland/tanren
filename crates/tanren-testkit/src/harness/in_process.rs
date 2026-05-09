//! Direct-`Handlers` harness — the legacy path the rest of the test
//! suite ran on before R-0001 sub-9. Kept as the fallback for
//! untagged scenarios and as the temporary stand-in for `@web` (until
//! PR 11 wires `playwright-bdd`) and `@tui` (until expectrl scraping
//! is hardened).

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use tanren_app_services::{
    ActiveAccountContext, ActiveAccountContextError, Clock, Handlers, Store,
};
use tanren_contract::{
    AcceptInvitationRequest, ListActiveAccountsRequest, SignInRequest, SignUpRequest,
    SignedInAccountView, SwitchActiveAccountRequest,
};
use tanren_identity_policy::AccountId;
use tanren_identity_policy::Argon2idVerifier;
use tanren_store::{AccountStore, EventEnvelope, NewInvitation};

use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind, HarnessResult,
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
    signed_in_account_ids: Vec<AccountId>,
    active_account_by_window: std::collections::HashMap<String, AccountId>,
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
            signed_in_account_ids: Vec::new(),
            active_account_by_window: std::collections::HashMap::new(),
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

    fn note_signed_in(&mut self, account_id: AccountId) {
        if !self.signed_in_account_ids.contains(&account_id) {
            self.signed_in_account_ids.push(account_id);
        }
        self.active_account_by_window
            .insert(default_window_key().to_owned(), account_id);
    }

    fn active_context_for_window(
        &self,
        window_id: Option<&str>,
    ) -> HarnessResult<ActiveAccountContext> {
        let Some(active_account_id) = self
            .active_account_by_window
            .get(normalize_window_id(window_id))
            .copied()
            .filter(|id| self.signed_in_account_ids.contains(id))
            .or_else(|| self.signed_in_account_ids.first().copied())
        else {
            return Err(HarnessError::Account(
                tanren_contract::AccountFailureReason::InvalidCredential,
                "no signed-in account in harness session".to_owned(),
            ));
        };
        ActiveAccountContext::from_account_ids(
            active_account_id,
            self.signed_in_account_ids.clone(),
        )
        .map_err(|err| translate_active_account_context_error(&err))
    }

    fn write_active_account_for_window(&mut self, window_id: Option<&str>, account_id: AccountId) {
        self.active_account_by_window
            .insert(normalize_window_id(window_id).to_owned(), account_id);
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
                self.note_signed_in(response.account.id);
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
                self.note_signed_in(response.account.id);
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
                self.note_signed_in(response.account.id);
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

    async fn list_active_accounts(&mut self) -> HarnessResult<Vec<SignedInAccountView>> {
        let context = self.active_context_for_window(None)?;
        match self
            .handlers
            .list_active_accounts(&self.store, &context, ListActiveAccountsRequest::default())
            .await
        {
            Ok(response) => Ok(response.accounts),
            Err(err) => Err(translate_app_error(err)),
        }
    }

    async fn switch_active_account(
        &mut self,
        target_account_id: AccountId,
    ) -> HarnessResult<Vec<SignedInAccountView>> {
        let context = self.active_context_for_window(None)?;
        match self
            .handlers
            .switch_active_account(
                &self.store,
                &context,
                SwitchActiveAccountRequest { target_account_id },
            )
            .await
        {
            Ok(response) => {
                self.write_active_account_for_window(None, response.active_account_id);
                Ok(response.accounts)
            }
            Err(err) => Err(translate_app_error(err)),
        }
    }

    async fn list_active_accounts_in_window(
        &mut self,
        window_id: &str,
    ) -> HarnessResult<Vec<SignedInAccountView>> {
        let context = self.active_context_for_window(Some(window_id))?;
        match self
            .handlers
            .list_active_accounts(&self.store, &context, ListActiveAccountsRequest::default())
            .await
        {
            Ok(response) => Ok(response.accounts),
            Err(err) => Err(translate_app_error(err)),
        }
    }

    async fn switch_active_account_in_window(
        &mut self,
        window_id: &str,
        target_account_id: AccountId,
    ) -> HarnessResult<Vec<SignedInAccountView>> {
        let context = self.active_context_for_window(Some(window_id))?;
        match self
            .handlers
            .switch_active_account(
                &self.store,
                &context,
                SwitchActiveAccountRequest { target_account_id },
            )
            .await
        {
            Ok(response) => {
                self.write_active_account_for_window(Some(window_id), response.active_account_id);
                Ok(response.accounts)
            }
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
        AppServiceError::InvalidInput(msg) => {
            HarnessError::Account(tanren_contract::AccountFailureReason::ValidationFailed, msg)
        }
        AppServiceError::Store(err) => HarnessError::Transport(format!("store: {err}")),
        _ => HarnessError::Transport("unknown app-service failure".to_owned()),
    }
}

fn translate_active_account_context_error(err: &ActiveAccountContextError) -> HarnessError {
    HarnessError::Account(
        tanren_contract::AccountFailureReason::ValidationFailed,
        err.to_string(),
    )
}

fn normalize_window_id(window_id: Option<&str>) -> &str {
    match window_id.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => value,
        None => default_window_key(),
    }
}

fn default_window_key() -> &'static str {
    "_default"
}
