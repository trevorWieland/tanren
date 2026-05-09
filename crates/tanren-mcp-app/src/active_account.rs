//! Per-session active-account context for the MCP tool surface.
//!
//! MCP has no cookie jar, so we keep a bounded signed-in account set in
//! memory per MCP session and project that into `ActiveAccountContext`
//! when callers list/switch active accounts.

use std::sync::Mutex;

use tanren_app_services::{ActiveAccountContext, ActiveAccountContextError};
use tanren_identity_policy::AccountId;

#[derive(Debug, Default)]
pub(crate) struct ActiveAccountSessionState {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    active_account_id: Option<AccountId>,
    signed_in_account_ids: Vec<AccountId>,
}

impl ActiveAccountSessionState {
    pub(crate) fn note_signed_in(&self, account_id: AccountId) {
        let mut state = self
            .inner
            .lock()
            .expect("active-account session mutex poisoned");
        if !state.signed_in_account_ids.contains(&account_id) {
            state.signed_in_account_ids.push(account_id);
        }
        state.active_account_id = Some(account_id);
    }

    pub(crate) fn context(
        &self,
    ) -> Result<Option<ActiveAccountContext>, ActiveAccountContextError> {
        let state = self
            .inner
            .lock()
            .expect("active-account session mutex poisoned");
        if state.signed_in_account_ids.is_empty() {
            return Ok(None);
        }
        let active_account_id = state
            .active_account_id
            .filter(|id| state.signed_in_account_ids.contains(id))
            .unwrap_or(state.signed_in_account_ids[0]);
        ActiveAccountContext::from_account_ids(
            active_account_id,
            state.signed_in_account_ids.clone(),
        )
        .map(Some)
    }

    pub(crate) fn set_active(&self, active_account_id: AccountId) {
        let mut state = self
            .inner
            .lock()
            .expect("active-account session mutex poisoned");
        state.active_account_id = Some(active_account_id);
    }
}
