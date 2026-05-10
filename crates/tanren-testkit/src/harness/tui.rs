//! `@tui` harness — drives the real `tanren-tui` binary in a pty.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use secrecy::ExposeSecret;
use tanren_contract::{
    AcceptInvitationRequest, AccountFailureReason, AccountView, SignInRequest, SignUpRequest,
    SignedInAccountView,
};
use tanren_identity_policy::{AccountId, Identifier};
use tanren_store::{AccountStore, EventEnvelope, NewInvitation, Store};

use self::driver::{
    expect_any_text, expect_text, run_tui_script, select_menu_index, send_down, send_enter,
    send_form_fields,
};
use super::api::{scenario_db_path, sqlite_url};
use super::api_codec::code_to_reason;
use super::cli::locate_workspace_binary;
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind, HarnessResult,
    HarnessSession,
};

mod driver;
mod projection;
mod session_state;
use self::session_state::{ActiveSession, TuiSessionFile};

const DEFAULT_WINDOW_ID: &str = "55555555-5555-4555-8555-555555555555";
/// `@tui` wire harness.
pub struct TuiHarness {
    store: Arc<Store>,
    db_path: PathBuf,
    db_url: String,
    binary: PathBuf,
    session_file: PathBuf,
    window_id: String,
    seeded_inviting_orgs: HashMap<String, tanren_identity_policy::OrgId>,
    known_accounts: HashMap<AccountId, AccountView>,
}

impl std::fmt::Debug for TuiHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TuiHarness")
            .field("db_path", &self.db_path)
            .field("binary", &self.binary)
            .finish_non_exhaustive()
    }
}

impl TuiHarness {
    pub async fn spawn() -> HarnessResult<Self> {
        let db_path = scenario_db_path("tui");
        let db_url = sqlite_url(&db_path);
        let store = Store::connect(&db_url)
            .await
            .map_err(|e| HarnessError::Transport(format!("connect store: {e}")))?;
        store
            .migrate()
            .await
            .map_err(|e| HarnessError::Transport(format!("migrate store: {e}")))?;

        let binary = locate_workspace_binary("tanren-tui")?;
        let session_file = db_path.with_extension("session.json");

        Ok(Self {
            store: Arc::new(store),
            db_path,
            db_url,
            binary,
            session_file,
            window_id: DEFAULT_WINDOW_ID.to_owned(),
            seeded_inviting_orgs: HashMap::new(),
            known_accounts: HashMap::new(),
        })
    }
}

impl Drop for TuiHarness {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.db_path);
        let _ = std::fs::remove_file(&self.session_file);
    }
}

#[async_trait]
impl AccountHarness for TuiHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Tui
    }

    async fn sign_up(&mut self, req: SignUpRequest) -> HarnessResult<HarnessSession> {
        run_tui_script(
            &self.binary,
            &self.db_url,
            &self.session_file,
            &self.window_id,
            None,
            "sign_up",
            |session, rendered| {
                select_menu_index(session, 0)?;
                expect_text(session, "Email:", rendered)?;
                send_form_fields(
                    session,
                    &[
                        req.email.as_str(),
                        req.password.expose_secret(),
                        &req.display_name,
                    ],
                )?;
                let matched =
                    expect_any_text(session, &["account_id:", "duplicate_identifier"], rendered)?;
                if matched != "account_id:" {
                    return Err(classify_account_failure(&matched));
                }
                Ok(())
            },
        )?;
        let active = self.active_session_for_window(&self.window_id)?;
        let account = AccountView {
            id: active.account_id,
            identifier: Identifier::from_email(&req.email),
            display_name: req.display_name.clone(),
            org: None,
        };
        self.remember_account(account.clone());
        Ok(HarnessSession {
            account_id: account.id,
            account,
            expires_at: Utc::now() + Duration::days(30),
            has_token: active.has_token,
        })
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        run_tui_script(
            &self.binary,
            &self.db_url,
            &self.session_file,
            &self.window_id,
            None,
            "sign_in",
            |session, rendered| {
                select_menu_index(session, 1)?;
                expect_text(session, "Email:", rendered)?;
                expect_text(session, "Password:", rendered)?;
                send_form_fields(session, &[req.email.as_str(), req.password.expose_secret()])?;
                let matched =
                    expect_any_text(session, &["account_id:", "invalid_credential"], rendered)?;
                if matched != "account_id:" {
                    return Err(classify_account_failure(&matched));
                }
                Ok(())
            },
        )?;
        let active = self.active_session_for_window(&self.window_id)?;
        let account = if let Some(existing) = self.known_accounts.get(&active.account_id) {
            existing.clone()
        } else {
            AccountView {
                id: active.account_id,
                identifier: Identifier::from_email(&req.email),
                display_name: String::new(),
                org: None,
            }
        };
        self.remember_account(account.clone());
        Ok(HarnessSession {
            account_id: account.id,
            account,
            expires_at: Utc::now() + Duration::days(30),
            has_token: active.has_token,
        })
    }

    async fn accept_invitation(
        &mut self,
        req: AcceptInvitationRequest,
    ) -> HarnessResult<HarnessAcceptance> {
        run_tui_script(
            &self.binary,
            &self.db_url,
            &self.session_file,
            &self.window_id,
            None,
            "accept_invitation",
            |session, rendered| {
                select_menu_index(session, 2)?;
                expect_text(session, "Invitation token:", rendered)?;
                send_form_fields(
                    session,
                    &[
                        req.invitation_token.as_str(),
                        req.email.as_str(),
                        req.password.expose_secret(),
                        &req.display_name,
                    ],
                )?;
                let matched = expect_any_text(
                    session,
                    &[
                        "account_id:",
                        "invitation_expired",
                        "invitation_already_consumed",
                        "invitation_not_found",
                        "invalid_credential",
                        "duplicate_identifier",
                    ],
                    rendered,
                )?;
                if matched != "account_id:" {
                    return Err(classify_account_failure(&matched));
                }
                Ok(())
            },
        )?;
        let active = self.active_session_for_window(&self.window_id)?;
        let joined_org = self
            .seeded_inviting_orgs
            .get(req.invitation_token.as_str())
            .copied()
            .ok_or_else(|| {
                HarnessError::Transport(format!(
                    "missing seeded invitation org for token {}",
                    req.invitation_token.as_str()
                ))
            })?;
        let account = AccountView {
            id: active.account_id,
            identifier: Identifier::from_email(&req.email),
            display_name: req.display_name.clone(),
            org: Some(joined_org),
        };
        self.remember_account(account.clone());

        Ok(HarnessAcceptance {
            session: HarnessSession {
                account_id: account.id,
                account,
                expires_at: Utc::now() + Duration::days(30),
                has_token: active.has_token,
            },
            joined_org,
        })
    }

    async fn seed_invitation(&mut self, fixture: HarnessInvitation) -> HarnessResult<()> {
        let token = fixture.token.as_str().to_owned();
        let _ = self
            .seeded_inviting_orgs
            .insert(token, fixture.inviting_org);
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

    async fn list_active_accounts(&mut self) -> HarnessResult<Vec<SignedInAccountView>> {
        let window_id = self.window_id.clone();
        self.list_active_accounts_in_window(&window_id).await
    }

    async fn list_active_accounts_in_window(
        &mut self,
        window_id: &str,
    ) -> HarnessResult<Vec<SignedInAccountView>> {
        let expected = self.signed_in_accounts_for_window(window_id)?;
        run_tui_script(
            &self.binary,
            &self.db_url,
            &self.session_file,
            window_id,
            None,
            "list_active_accounts",
            |session, rendered| {
                select_menu_index(session, 3)?;
                expect_text(session, "signed-in accounts:", rendered)?;
                for entry in &expected {
                    expect_text(
                        session,
                        &format!("{} {}", marker(entry.is_active), entry.account.id),
                        rendered,
                    )?;
                }
                Ok(())
            },
        )?;
        Ok(expected)
    }

    async fn switch_active_account(
        &mut self,
        target_account_id: AccountId,
    ) -> HarnessResult<Vec<SignedInAccountView>> {
        let window_id = self.window_id.clone();
        self.switch_active_account_in_window(&window_id, target_account_id)
            .await
    }

    async fn switch_active_account_in_window(
        &mut self,
        window_id: &str,
        target_account_id: AccountId,
    ) -> HarnessResult<Vec<SignedInAccountView>> {
        let before = self.signed_in_accounts_for_window(window_id)?;
        let target_idx = before
            .iter()
            .position(|entry| entry.account.id == target_account_id);

        if target_idx.is_none() {
            run_tui_script(
                &self.binary,
                &self.db_url,
                &self.session_file,
                window_id,
                Some(target_account_id),
                "switch_active_account_rejected",
                |session, rendered| {
                    select_menu_index(session, 4)?;
                    expect_text(session, "Choose an account:", rendered)?;
                    send_enter(session)?;
                    let matched = expect_any_text(
                        session,
                        &["target_account_not_signed_in", "validation_failed"],
                        rendered,
                    )?;
                    if matched != "target_account_not_signed_in" {
                        return Err(classify_account_failure(&matched));
                    }
                    Ok(())
                },
            )?;
            return Err(HarnessError::Account(
                AccountFailureReason::TargetAccountNotSignedIn,
                "target_account_not_signed_in".to_owned(),
            ));
        }

        let current_idx = before
            .iter()
            .position(|entry| entry.is_active)
            .ok_or_else(|| {
                HarnessError::Transport(
                    "tui signed-in projection has no active account before switch".to_owned(),
                )
            })?;
        let target_idx = target_idx.ok_or_else(|| {
            HarnessError::Transport("missing target account index for switch".to_owned())
        })?;
        let steps_down = (target_idx + before.len() - current_idx) % before.len();

        run_tui_script(
            &self.binary,
            &self.db_url,
            &self.session_file,
            window_id,
            None,
            "switch_active_account",
            |session, rendered| {
                select_menu_index(session, 4)?;
                expect_text(session, "Choose an account:", rendered)?;
                for _ in 0..steps_down {
                    send_down(session)?;
                }
                send_enter(session)?;
                let matched = expect_any_text(
                    session,
                    &[
                        "active_account_id:",
                        "target_account_not_signed_in",
                        "validation_failed",
                    ],
                    rendered,
                )?;
                if matched != "active_account_id:" {
                    return Err(classify_account_failure(&matched));
                }
                expect_text(session, &target_account_id.to_string(), rendered)?;
                Ok(())
            },
        )?;

        let after = self.signed_in_accounts_for_window(window_id)?;
        if !after
            .iter()
            .any(|entry| entry.account.id == target_account_id && entry.is_active)
        {
            return Err(HarnessError::Transport(format!(
                "tui switch did not activate target account {target_account_id}"
            )));
        }
        Ok(after)
    }

    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<EventEnvelope>> {
        AccountStore::recent_events(self.store.as_ref(), limit)
            .await
            .map_err(|e| HarnessError::Transport(format!("recent_events: {e}")))
    }
}

fn marker(active: bool) -> &'static str {
    if active { "*" } else { "-" }
}

fn classify_account_failure(code: &str) -> HarnessError {
    if let Some(reason) = code_to_reason(code) {
        HarnessError::Account(reason, code.to_owned())
    } else {
        HarnessError::Transport(format!("unrecognized tui failure code: {code}"))
    }
}
