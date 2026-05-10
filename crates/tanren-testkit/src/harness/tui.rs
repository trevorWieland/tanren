//! `@tui` harness — drives the real `tanren-tui` binary in a pty.
use std::sync::Arc;

use super::api::{scenario_db_path, sqlite_url};
use super::tui_binary::locate_or_build_tui_binary;
use super::tui_driver::{TuiDriver, TuiMenuChoice, TuiTranscript};
use super::tui_errors::{parse_account_failure, parse_role_failure};
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind, HarnessResult,
    HarnessSession, RoleHarnessError, RoleHarnessResult,
};
use async_trait::async_trait;
use chrono::{Duration as ChronoDuration, Utc};
use secrecy::ExposeSecret;
use tanren_app_services::Store;
use tanren_contract::{
    AcceptInvitationRequest, AccountView, RoleFailureReason, SignInRequest, SignUpRequest,
};
use tanren_identity_policy::AccountId;
use tanren_store::{AccountStore, EventEnvelope, NewInvitation};

/// `@tui` wire harness.
pub struct TuiHarness {
    store: Arc<Store>,
    db_path: std::path::PathBuf,
    driver: TuiDriver,
    role_actor: Option<AccountId>,
    auth_credentials: Option<(String, String)>,
    last_transcript: Option<String>,
}

impl std::fmt::Debug for TuiHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TuiHarness")
            .field("db_path", &self.db_path)
            .finish_non_exhaustive()
    }
}

impl TuiHarness {
    /// Construct a fresh TUI harness. Connects + migrates a per-scenario
    /// `SQLite` database and prepares a PTY driver for `tanren-tui`.
    pub async fn spawn() -> HarnessResult<Self> {
        let db_path = scenario_db_path("tui");
        let database_url = sqlite_url(&db_path);
        let store = Store::connect(&database_url)
            .await
            .map_err(|e| HarnessError::Transport(format!("connect store: {e}")))?;
        store
            .migrate()
            .await
            .map_err(|e| HarnessError::Transport(format!("migrate store: {e}")))?;

        let binary = locate_or_build_tui_binary()?;
        let driver = TuiDriver::new(binary, database_url);
        driver.probe_startup()?;

        Ok(Self {
            store: Arc::new(store),
            db_path,
            driver,
            role_actor: None,
            auth_credentials: None,
            last_transcript: None,
        })
    }

    async fn read_account_view_by_email(
        &self,
        email: &tanren_identity_policy::Email,
    ) -> HarnessResult<AccountView> {
        let Some(record) = self
            .store
            .find_account_by_email(email)
            .await
            .map_err(|e| HarnessError::Transport(format!("read account by email: {e}")))?
        else {
            return Err(HarnessError::Transport(
                "tui submit succeeded but account row was not found".to_owned(),
            ));
        };
        Ok(AccountView {
            id: record.id,
            identifier: record.identifier,
            display_name: record.display_name,
            org: record.org_id,
        })
    }

    fn ensure_role_outcome(
        transcript: &TuiTranscript,
        success_label: &str,
    ) -> RoleHarnessResult<()> {
        if transcript.contains(success_label) {
            return Ok(());
        }
        if matches!(
            success_label,
            "Role created" | "Role updated" | "Role deleted"
        ) && transcript.line_value("role_id").is_some()
        {
            return Ok(());
        }
        if success_label == "Role applied" && transcript.line_value("grants_created").is_some() {
            return Ok(());
        }
        if let Some(err) = parse_role_failure(transcript) {
            return Err(err);
        }
        Err(RoleHarnessError::Transport(format!(
            "unexpected tui role transcript for `{success_label}`: {}",
            transcript.text
        )))
    }

    fn role_transport(err: HarnessError) -> RoleHarnessError {
        match err {
            HarnessError::Transport(message) | HarnessError::Account(_, message) => {
                if message.contains("validation_failed") || message.contains("validationfailed") {
                    RoleHarnessError::Role(
                        RoleFailureReason::ValidationFailed,
                        "validation_failed".to_owned(),
                    )
                } else {
                    RoleHarnessError::Transport(message)
                }
            }
        }
    }

    fn store_transcript(&mut self, transcript: &TuiTranscript) {
        self.last_transcript = Some(transcript.text.clone());
    }

    fn submit_role_form(
        &self,
        choice: TuiMenuChoice,
        form_title: &str,
        values: &[String],
    ) -> RoleHarnessResult<TuiTranscript> {
        let mut last_error: Option<RoleHarnessError> = None;
        for _attempt in 0..2 {
            let submit = if let Some((email, password)) = self.auth_credentials.as_ref() {
                self.driver
                    .submit_form_with_sign_in(email, password, choice, form_title, values)
            } else {
                self.driver.submit_form(choice, form_title, values)
            };
            match submit {
                Ok(transcript) => return Ok(transcript),
                Err(err) => {
                    let role_err = Self::role_transport(err);
                    if !matches!(role_err, RoleHarnessError::Transport(_)) {
                        return Err(role_err);
                    }
                    last_error = Some(role_err);
                }
            }
        }
        Err(last_error.unwrap_or_else(|| {
            RoleHarnessError::Transport("tui role form submission failed".to_owned())
        }))
    }

    fn remember_auth(
        &mut self,
        email: &tanren_identity_policy::Email,
        password: &secrecy::SecretString,
        account_id: AccountId,
    ) {
        self.role_actor = Some(account_id);
        self.auth_credentials = Some((
            email.as_str().to_owned(),
            password.expose_secret().to_owned(),
        ));
    }
}

impl Drop for TuiHarness {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.db_path);
    }
}

#[async_trait]
impl AccountHarness for TuiHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Tui
    }

    async fn sign_up(&mut self, req: SignUpRequest) -> HarnessResult<HarnessSession> {
        let transcript = self.driver.submit_form(
            TuiMenuChoice::SignUp,
            "Sign up",
            &[
                req.email.as_str().to_owned(),
                req.password.expose_secret().to_owned(),
                req.display_name.clone(),
            ],
        )?;

        let sign_up_ok =
            transcript.contains("Account created") || transcript.line_value("account_id").is_some();
        if !sign_up_ok {
            if let Some(err) = parse_account_failure(&transcript) {
                return Err(err);
            }
            return Err(HarnessError::Transport(format!(
                "unexpected sign-up transcript: {}",
                transcript.text
            )));
        }

        let account = self.read_account_view_by_email(&req.email).await?;
        let account_id = account.id;
        let has_token = transcript.contains("session token");

        self.remember_auth(&req.email, &req.password, account_id);

        Ok(HarnessSession {
            account,
            account_id,
            expires_at: Utc::now() + ChronoDuration::days(30),
            has_token,
        })
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        let transcript = self.driver.submit_form(
            TuiMenuChoice::SignIn,
            "Sign in",
            &[
                req.email.as_str().to_owned(),
                req.password.expose_secret().to_owned(),
            ],
        )?;

        let signed_in_ok =
            transcript.contains("Signed in") || transcript.line_value("account_id").is_some();
        if !signed_in_ok {
            if let Some(err) = parse_account_failure(&transcript) {
                return Err(err);
            }
            return Err(HarnessError::Transport(format!(
                "unexpected sign-in transcript: {}",
                transcript.text
            )));
        }

        let account = self.read_account_view_by_email(&req.email).await?;
        let account_id = account.id;
        let has_token = transcript.contains("session token");

        self.remember_auth(&req.email, &req.password, account_id);

        Ok(HarnessSession {
            account,
            account_id,
            expires_at: Utc::now() + ChronoDuration::days(30),
            has_token,
        })
    }

    async fn accept_invitation(
        &mut self,
        req: AcceptInvitationRequest,
    ) -> HarnessResult<HarnessAcceptance> {
        let transcript = self.driver.submit_form(
            TuiMenuChoice::AcceptInvitation,
            "Accept invitation",
            &[
                req.invitation_token.as_str().to_owned(),
                req.email.as_str().to_owned(),
                req.password.expose_secret().to_owned(),
                req.display_name,
            ],
        )?;

        let accepted_ok = transcript.contains("Invitation accepted")
            || transcript.line_value("account_id").is_some();
        if !accepted_ok {
            if let Some(err) = parse_account_failure(&transcript) {
                return Err(err);
            }
            return Err(HarnessError::Transport(format!(
                "unexpected accept-invitation transcript: {}",
                transcript.text
            )));
        }

        let account = self.read_account_view_by_email(&req.email).await?;
        let account_id = account.id;
        let joined_org = account.org.ok_or_else(|| {
            HarnessError::Transport("accepted invitation account is missing org".to_owned())
        })?;
        let has_token = transcript.contains("session token");

        self.remember_auth(&req.email, &req.password, account_id);

        Ok(HarnessAcceptance {
            session: HarnessSession {
                account,
                account_id,
                expires_at: Utc::now() + ChronoDuration::days(30),
                has_token,
            },
            joined_org,
        })
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
        AccountStore::recent_events(self.store.as_ref(), limit)
            .await
            .map_err(|e| HarnessError::Transport(format!("recent_events: {e}")))
    }

    async fn drain(&mut self) {
        // The TUI driver spawns a PTY child per form submission
        // and kills it in finish_session, so there is no persistent
        // child process to tear down. The Drop impl cleans up the
        // DB file. This override exists so the After hook can
        // trigger the Arc<Store> release path.
    }
}

#[path = "tui/role_impl.rs"]
mod role_impl;
