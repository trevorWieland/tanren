use std::collections::HashSet;

use tanren_contract::{AccountFailureReason, ActiveAccountView, SignedInAccountView};

use super::{ActiveSession, HarnessError, HarnessResult, TuiHarness, TuiSessionFile};

impl TuiHarness {
    pub(super) fn read_session_file(&self) -> HarnessResult<TuiSessionFile> {
        if !self.session_file.exists() {
            return Ok(TuiSessionFile::default());
        }
        let raw = std::fs::read_to_string(&self.session_file)
            .map_err(|e| HarnessError::Transport(format!("read session file: {e}")))?;
        if raw.trim().is_empty() {
            return Ok(TuiSessionFile::default());
        }
        serde_json::from_str(&raw)
            .map_err(|e| HarnessError::Transport(format!("parse session file: {e}")))
    }

    pub(super) fn active_session_for_window(
        &self,
        window_id: &str,
    ) -> HarnessResult<ActiveSession> {
        let session = self.read_session_file()?;
        if session.signed_in.is_empty() {
            return Err(HarnessError::Account(
                AccountFailureReason::InvalidCredential,
                "invalid_credential: no signed-in account in session file".to_owned(),
            ));
        }
        let mut seen = HashSet::new();
        let ordered_ids = session
            .signed_in
            .iter()
            .map(|entry| entry.account_id)
            .filter(|id| seen.insert(*id))
            .collect::<Vec<_>>();

        let default_active = ordered_ids[0];
        let active = session
            .active_account_by_window
            .get(window_id)
            .copied()
            .filter(|id| ordered_ids.contains(id))
            .unwrap_or(default_active);
        let token = session
            .signed_in
            .iter()
            .find(|entry| entry.account_id == active)
            .map(|entry| entry.token.expose_secret().to_owned())
            .unwrap_or_default();
        Ok(ActiveSession {
            account_id: active,
            has_token: !token.is_empty(),
        })
    }

    pub(super) fn signed_in_accounts_for_window(
        &self,
        window_id: &str,
    ) -> HarnessResult<Vec<SignedInAccountView>> {
        let session = self.read_session_file()?;
        if session.signed_in.is_empty() {
            return Err(HarnessError::Account(
                AccountFailureReason::InvalidCredential,
                "invalid_credential: no signed-in account in session file".to_owned(),
            ));
        }
        let mut seen = HashSet::new();
        let ordered_ids = session
            .signed_in
            .iter()
            .map(|entry| entry.account_id)
            .filter(|id| seen.insert(*id))
            .collect::<Vec<_>>();
        let default_active = ordered_ids[0];
        let active = session
            .active_account_by_window
            .get(window_id)
            .copied()
            .filter(|id| ordered_ids.contains(id))
            .unwrap_or(default_active);

        let mut accounts = Vec::with_capacity(ordered_ids.len());
        for account_id in ordered_ids {
            let account = self.known_accounts.get(&account_id).ok_or_else(|| {
                HarnessError::Transport(format!(
                    "tui session references unknown account id {account_id}; no prior account metadata recorded"
                ))
            })?;
            accounts.push(SignedInAccountView {
                account: ActiveAccountView {
                    id: account.id,
                    display_name: account.display_name.clone(),
                    org: account.org,
                },
                is_active: account_id == active,
            });
        }
        assert_compact_assumptions(&accounts)?;
        Ok(accounts)
    }

    pub(super) fn remember_account(&mut self, account: tanren_contract::AccountView) {
        let _ = self.known_accounts.insert(account.id, account);
    }
}

fn assert_compact_assumptions(accounts: &[SignedInAccountView]) -> HarnessResult<()> {
    if accounts.len() > 9 {
        return Err(HarnessError::Transport(
            "tui compact switcher exceeded 9 visible accounts".to_owned(),
        ));
    }
    let active_count = accounts.iter().filter(|entry| entry.is_active).count();
    if active_count != 1 {
        return Err(HarnessError::Transport(format!(
            "tui compact switcher expected exactly one active account, got {active_count}"
        )));
    }
    Ok(())
}
