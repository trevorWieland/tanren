//! Per-session active-account context for the MCP tool surface.
//!
//! MCP has no cookie jar, so we keep a bounded signed-in account set in
//! memory per MCP session and project that into `ActiveAccountContext`
//! when callers list/switch active accounts.

use std::collections::HashSet;
use std::error::Error as StdError;
use std::sync::Mutex;

use tanren_app_services::{AccountStore, ActiveAccountContext, ActiveAccountContextError, Clock};
use tanren_identity_policy::{AccountId, SessionToken};

#[derive(Debug, Default)]
pub(crate) struct ActiveAccountSessionState {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    active_account_id: Option<AccountId>,
    signed_in: Vec<SignedInSession>,
}

#[derive(Debug, Clone)]
struct SignedInSession {
    account_id: AccountId,
    token: SessionToken,
}

#[derive(Debug)]
pub(crate) enum ActiveAccountSessionError {
    Validation(ActiveAccountContextError),
    Store(String),
}

impl ActiveAccountSessionState {
    pub(crate) fn note_signed_in(&self, account_id: AccountId, token: SessionToken) {
        let mut state = self
            .inner
            .lock()
            .expect("active-account session mutex poisoned");
        if let Some(existing) = state
            .signed_in
            .iter_mut()
            .find(|entry| entry.account_id == account_id)
        {
            existing.token = token;
        } else {
            state.signed_in.push(SignedInSession { account_id, token });
        }
        state.active_account_id = Some(account_id);
    }

    pub(crate) async fn context<S>(
        &self,
        store: &S,
        clock: &Clock,
    ) -> Result<Option<ActiveAccountContext>, ActiveAccountSessionError>
    where
        S: AccountStore + ?Sized,
    {
        let snapshot = self
            .inner
            .lock()
            .expect("active-account session mutex poisoned")
            .signed_in
            .clone();

        if snapshot.is_empty() {
            return Ok(None);
        }

        let now = clock.now();
        let mut validated = Vec::with_capacity(snapshot.len());
        let mut seen = HashSet::new();
        for entry in snapshot {
            match store.validate_session_token(&entry.token, now).await {
                Ok(account) => {
                    if seen.insert(account.id) {
                        validated.push(SignedInSession {
                            account_id: account.id,
                            token: entry.token,
                        });
                    }
                }
                Err(err) => {
                    if StdError::source(&err).is_some() {
                        return Err(ActiveAccountSessionError::Store(err.to_string()));
                    }
                }
            }
        }

        let mut state = self
            .inner
            .lock()
            .expect("active-account session mutex poisoned");
        state.signed_in = validated;
        if state.signed_in.is_empty() {
            state.active_account_id = None;
            return Ok(None);
        }

        let signed_in_account_ids = state
            .signed_in
            .iter()
            .map(|entry| entry.account_id)
            .collect::<Vec<_>>();
        let active_account_id = state
            .active_account_id
            .filter(|id| signed_in_account_ids.contains(id))
            .unwrap_or(signed_in_account_ids[0]);
        state.active_account_id = Some(active_account_id);

        ActiveAccountContext::from_account_ids(active_account_id, signed_in_account_ids)
            .map(Some)
            .map_err(ActiveAccountSessionError::Validation)
    }

    pub(crate) fn set_active(&self, active_account_id: AccountId) {
        let mut state = self
            .inner
            .lock()
            .expect("active-account session mutex poisoned");
        state.active_account_id = Some(active_account_id);
    }
}
