//! Direct-`Handlers` harness — the legacy path the rest of the test
//! suite ran on before R-0001 sub-9. Kept as the fallback for
//! untagged scenarios and as the temporary stand-in for `@web` (until
//! PR 11 wires `playwright-bdd`) and `@tui` (until expectrl scraping
//! is hardened).

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use secrecy::SecretString;
use tanren_app_services::{AuthenticatedActor, Clock, Handlers, Store};
use tanren_configuration_secrets::{
    CredentialId, CredentialKind, UserSettingKey, UserSettingValue,
};
use tanren_contract::{
    AcceptInvitationRequest, CreateCredentialRequest, GetUserConfigRequest,
    RemoveCredentialRequest, SetUserConfigRequest, SignInRequest, SignUpRequest,
    UpdateCredentialRequest,
};
use tanren_identity_policy::{AccountId, Argon2idVerifier};
use tanren_store::{AccountStore, EventEnvelope, NewInvitation};

use super::{
    AccountHarness, HarnessAcceptance, HarnessConfigEntry, HarnessCredential, HarnessError,
    HarnessInvitation, HarnessKind, HarnessResult, HarnessSession,
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

    async fn set_user_config(
        &mut self,
        account_id: AccountId,
        key: UserSettingKey,
        value: UserSettingValue,
    ) -> HarnessResult<HarnessConfigEntry> {
        let actor = AuthenticatedActor::from_account_id(account_id);
        match self
            .handlers
            .set_user_config(&self.store, &actor, SetUserConfigRequest { key, value })
            .await
        {
            Ok(resp) => Ok(HarnessConfigEntry {
                key: resp.entry.key,
                value: resp.entry.value,
                updated_at: resp.entry.updated_at,
            }),
            Err(err) => Err(translate_app_error(err)),
        }
    }

    async fn get_user_config(
        &mut self,
        account_id: AccountId,
        key: UserSettingKey,
    ) -> HarnessResult<Option<HarnessConfigEntry>> {
        let actor = AuthenticatedActor::from_account_id(account_id);
        match self
            .handlers
            .get_user_config(&self.store, &actor, GetUserConfigRequest { key })
            .await
        {
            Ok(resp) => Ok(resp.entry.map(|e| HarnessConfigEntry {
                key: e.key,
                value: e.value,
                updated_at: e.updated_at,
            })),
            Err(err) => Err(translate_app_error(err)),
        }
    }

    async fn list_user_config(
        &mut self,
        account_id: AccountId,
    ) -> HarnessResult<Vec<HarnessConfigEntry>> {
        let actor = AuthenticatedActor::from_account_id(account_id);
        match self.handlers.list_user_config(&self.store, &actor).await {
            Ok(resp) => Ok(resp
                .entries
                .into_iter()
                .map(|e| HarnessConfigEntry {
                    key: e.key,
                    value: e.value,
                    updated_at: e.updated_at,
                })
                .collect()),
            Err(err) => Err(translate_app_error(err)),
        }
    }

    async fn attempt_get_other_user_config(
        &mut self,
        actor_account_id: AccountId,
        target_account_id: AccountId,
        key: UserSettingKey,
    ) -> HarnessResult<Option<HarnessConfigEntry>> {
        let actor = AuthenticatedActor::from_account_id(actor_account_id);
        match self
            .handlers
            .get_user_config(&self.store, &actor, GetUserConfigRequest { key })
            .await
        {
            Ok(resp) => {
                let _ = target_account_id;
                Ok(resp.entry.map(|e| HarnessConfigEntry {
                    key: e.key,
                    value: e.value,
                    updated_at: e.updated_at,
                }))
            }
            Err(err) => Err(translate_app_error(err)),
        }
    }

    async fn create_credential(
        &mut self,
        account_id: AccountId,
        kind: CredentialKind,
        name: String,
        secret: SecretString,
    ) -> HarnessResult<HarnessCredential> {
        let actor = AuthenticatedActor::from_account_id(account_id);
        match self
            .handlers
            .create_credential(
                &self.store,
                &actor,
                CreateCredentialRequest {
                    kind,
                    name,
                    description: None,
                    provider: None,
                    value: secret,
                },
            )
            .await
        {
            Ok(resp) => Ok(HarnessCredential {
                id: resp.credential.id,
                name: resp.credential.name,
                kind: resp.credential.kind,
                scope: resp.credential.scope,
                present: resp.credential.present,
            }),
            Err(err) => Err(translate_app_error(err)),
        }
    }

    async fn list_credentials(
        &mut self,
        account_id: AccountId,
    ) -> HarnessResult<Vec<HarnessCredential>> {
        let actor = AuthenticatedActor::from_account_id(account_id);
        match self.handlers.list_credentials(&self.store, &actor).await {
            Ok(resp) => Ok(resp
                .credentials
                .into_iter()
                .map(|c| HarnessCredential {
                    id: c.id,
                    name: c.name,
                    kind: c.kind,
                    scope: c.scope,
                    present: c.present,
                })
                .collect()),
            Err(err) => Err(translate_app_error(err)),
        }
    }

    async fn attempt_update_credential(
        &mut self,
        account_id: AccountId,
        credential_id: CredentialId,
        secret: SecretString,
    ) -> HarnessResult<HarnessCredential> {
        let actor = AuthenticatedActor::from_account_id(account_id);
        match self
            .handlers
            .update_credential(
                &self.store,
                &actor,
                UpdateCredentialRequest {
                    id: credential_id,
                    name: None,
                    description: None,
                    value: secret,
                },
            )
            .await
        {
            Ok(resp) => Ok(HarnessCredential {
                id: resp.credential.id,
                name: resp.credential.name,
                kind: resp.credential.kind,
                scope: resp.credential.scope,
                present: resp.credential.present,
            }),
            Err(err) => Err(translate_app_error(err)),
        }
    }

    async fn attempt_remove_credential(
        &mut self,
        account_id: AccountId,
        credential_id: CredentialId,
    ) -> HarnessResult<bool> {
        let actor = AuthenticatedActor::from_account_id(account_id);
        match self
            .handlers
            .remove_credential(
                &self.store,
                &actor,
                RemoveCredentialRequest { id: credential_id },
            )
            .await
        {
            Ok(resp) => Ok(resp.removed),
            Err(err) => Err(translate_app_error(err)),
        }
    }
}

fn translate_app_error(err: tanren_app_services::AppServiceError) -> HarnessError {
    use tanren_app_services::AppServiceError;
    match err {
        AppServiceError::Account(reason) => HarnessError::Account(reason, reason.code().to_owned()),
        AppServiceError::Configuration(reason) => {
            HarnessError::Configuration(reason, reason.code().to_owned())
        }
        AppServiceError::InvalidInput(msg) => {
            HarnessError::Transport(format!("invalid_input: {msg}"))
        }
        AppServiceError::Store(err) => HarnessError::Transport(format!("store: {err}")),
        _ => HarnessError::Transport("unknown app-service failure".to_owned()),
    }
}
