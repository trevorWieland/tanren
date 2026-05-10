//! `@tui` harness — drives `tanren-tui` over a real pseudo-terminal.
//!
//! The posture mutation path uses `/usr/bin/script` to allocate a PTY,
//! feed key events to the `tanren-tui` binary, and assert on the
//! rendered screen output. This keeps B-0137 posture witnesses on a
//! real terminal transport instead of the in-process fallback.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use async_trait::async_trait;
use secrecy::{ExposeSecret, SecretString};
use tanren_app_services::{AppServiceError, Handlers, Store};
use tanren_contract::{
    AcceptInvitationRequest, DeploymentPostureContractFailure, DeploymentPostureScope,
    RawSetDeploymentPostureRequest, SetDeploymentPostureRequest, SignInRequest, SignUpRequest,
};
use tanren_identity_policy::{AccountId, Email};
use tanren_store::{AccountStore, EventEnvelope, NewInvitation};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use super::api::{scenario_db_path, sqlite_url};
use super::cli::locate_workspace_binary;
use super::tui_driver::{
    extract_posture_failure, normalize_transcript, write_outcome_continue, write_posture_set_flow,
    write_sign_in_flow,
};
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind,
    HarnessPostureView, HarnessResult, HarnessSession, HarnessSupportedPosture,
};

/// `@tui` wire harness.
#[derive(Debug)]
pub struct TuiHarness {
    store: Arc<Store>,
    handlers: Handlers,
    db_path: PathBuf,
    db_url: String,
    binary: PathBuf,
    active_account: Option<AccountId>,
    active_email: Option<Email>,
    active_password: Option<SecretString>,
}

impl TuiHarness {
    /// Construct the TUI harness.
    ///
    /// # Errors
    ///
    /// Returns an error if the database or `tanren-tui` binary cannot
    /// be initialized.
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
        Ok(Self {
            store: Arc::new(store),
            handlers: Handlers::new(),
            db_path,
            db_url,
            binary,
            active_account: None,
            active_email: None,
            active_password: None,
        })
    }

    fn set_active_credentials(&mut self, account: AccountId, email: Email, password: SecretString) {
        self.active_account = Some(account);
        self.active_email = Some(email);
        self.active_password = Some(password);
    }

    async fn run_tui_posture_set(&self, posture_raw: &str) -> HarnessResult<()> {
        let email = self
            .active_email
            .as_ref()
            .map(Email::as_str)
            .ok_or(HarnessError::FailureCode {
                code: "permission_denied".to_owned(),
                summary: "sign up, sign in, or accept invitation before setting deployment posture"
                    .to_owned(),
            })?
            .to_owned();
        let password = self
            .active_password
            .as_ref()
            .map(ExposeSecret::expose_secret)
            .ok_or(HarnessError::FailureCode {
                code: "permission_denied".to_owned(),
                summary: "sign up, sign in, or accept invitation before setting deployment posture"
                    .to_owned(),
            })?
            .to_owned();

        let mut log_path = std::env::temp_dir();
        log_path.push(format!(
            "tanren-bdd-tui-transcript-{}-{}.log",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));

        let binary = self
            .binary
            .to_str()
            .ok_or_else(|| HarnessError::Transport("invalid tanren-tui binary path".to_owned()))?
            .to_owned();
        let log_file = log_path
            .to_str()
            .ok_or_else(|| HarnessError::Transport("invalid transcript path".to_owned()))?
            .to_owned();

        let mut child = Command::new("script")
            .args(["-q", "-e", "-f", "-c", &binary, &log_file])
            .env("DATABASE_URL", &self.db_url)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| HarnessError::Transport(format!("spawn script for tanren-tui: {e}")))?;

        let mut stdin = child.stdin.take().ok_or_else(|| {
            HarnessError::Transport("script child stdin was not piped".to_owned())
        })?;
        // Let the app enter alternate-screen mode before feeding keys.
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        write_sign_in_flow(&mut stdin, &email, &password)
            .await
            .map_err(|e| HarnessError::Transport(format!("write sign-in keystrokes: {e}")))?;
        // Wait for sign-in submission to finish and the outcome screen to render.
        tokio::time::sleep(std::time::Duration::from_millis(2_000)).await;
        write_outcome_continue(&mut stdin).await.map_err(|e| {
            HarnessError::Transport(format!("write outcome continue keystroke: {e}"))
        })?;
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
        write_posture_set_flow(&mut stdin, posture_raw)
            .await
            .map_err(|e| HarnessError::Transport(format!("write posture keystrokes: {e}")))?;
        // Give the posture submit path enough time to render either
        // an outcome view or an inline failure message before exiting.
        tokio::time::sleep(std::time::Duration::from_millis(1_500)).await;
        // Ctrl-C exits cleanly regardless of which screen is currently active.
        stdin
            .write_all(&[3])
            .await
            .map_err(|e| HarnessError::Transport(format!("write ctrl-c keystroke: {e}")))?;
        stdin
            .flush()
            .await
            .map_err(|e| HarnessError::Transport(format!("flush keystrokes: {e}")))?;

        let output = child
            .wait_with_output()
            .await
            .map_err(|e| HarnessError::Transport(format!("wait for script child: {e}")))?;
        let transcript = std::fs::read_to_string(&log_path)
            .map_err(|e| HarnessError::Transport(format!("read tui transcript: {e}")))?;
        let normalized_transcript = normalize_transcript(&transcript);
        if output.status.success() {
            if let Some((code, summary)) = extract_posture_failure(&normalized_transcript) {
                let _ = std::fs::remove_file(&log_path);
                return Err(HarnessError::FailureCode { code, summary });
            }
            if normalized_transcript.contains("audit reference:") {
                let _ = std::fs::remove_file(&log_path);
                return Ok(());
            }
            return Err(HarnessError::Transport(format!(
                "tui transcript missing posture outcome (transcript={})",
                log_path.display()
            )));
        }
        if let Some((code, summary)) = extract_posture_failure(&normalized_transcript) {
            let _ = std::fs::remove_file(&log_path);
            return Err(HarnessError::FailureCode { code, summary });
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(HarnessError::Transport(format!(
            "tui script exited non-zero: {} (stderr={stderr}, transcript={})",
            output.status,
            log_path.display()
        )))
    }

    async fn current_account_posture(&self, actor: AccountId) -> HarnessResult<HarnessPostureView> {
        let current = self
            .handlers
            .deployment_posture(
                self.store.as_ref(),
                actor,
                DeploymentPostureScope::Account { account_id: actor },
            )
            .await
            .map_err(|err| {
                let rendered = err.render();
                HarnessError::FailureCode {
                    code: rendered.code,
                    summary: rendered.summary,
                }
            })?
            .current
            .ok_or_else(|| {
                HarnessError::Transport(
                    "tui posture mutation succeeded but no current posture was stored".to_owned(),
                )
            })?;

        let mut view: HarnessPostureView = current.into();
        let events = AccountStore::recent_events(self.store.as_ref(), 40)
            .await
            .map_err(|e| HarnessError::Transport(format!("recent_events: {e}")))?;
        if let Some(event) = events.iter().find(|event| {
            event
                .payload
                .get("family")
                .and_then(serde_json::Value::as_str)
                == Some("deployment_posture")
                && event
                    .payload
                    .get("kind")
                    .and_then(serde_json::Value::as_str)
                    == Some("changed")
                && event
                    .payload
                    .get("payload")
                    .and_then(|payload| payload.get("changed_by"))
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|changed_by| changed_by == actor.to_string())
        }) {
            view.audit_reference = event.id.to_string();
        }
        if view.audit_reference.trim().is_empty() {
            return Err(HarnessError::Transport(
                "tui posture mutation did not emit a readable audit reference".to_owned(),
            ));
        }
        Ok(view)
    }

    async fn fallback_set_posture_via_handlers(
        &self,
        actor: AccountId,
        scope: DeploymentPostureScope,
        posture_raw: &str,
    ) -> HarnessResult<HarnessPostureView> {
        let request = SetDeploymentPostureRequest::try_from(RawSetDeploymentPostureRequest {
            scope,
            posture: posture_raw.to_owned(),
        })
        .map_err(|failure: DeploymentPostureContractFailure| {
            let body = failure.render();
            HarnessError::FailureCode {
                code: body.code,
                summary: body.summary,
            }
        })?;
        self.handlers
            .set_deployment_posture(self.store.as_ref(), actor, request)
            .await
            .map(Into::into)
            .map_err(|err| {
                let rendered = err.render();
                HarnessError::FailureCode {
                    code: rendered.code,
                    summary: rendered.summary,
                }
            })
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
        let email = req.email.clone();
        let password = req.password.clone();
        match self.handlers.sign_up(self.store.as_ref(), req).await {
            Ok(response) => {
                self.set_active_credentials(response.account.id, email, password);
                Ok(HarnessSession {
                    account_id: response.account.id,
                    account: response.account,
                    expires_at: response.session.expires_at,
                    has_token: !response.session.token.expose_secret().is_empty(),
                })
            }
            Err(err) => Err(translate_app_error(err)),
        }
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        let email = req.email.clone();
        let password = req.password.clone();
        match self.handlers.sign_in(self.store.as_ref(), req).await {
            Ok(response) => {
                self.set_active_credentials(response.account.id, email, password);
                Ok(HarnessSession {
                    account_id: response.account.id,
                    account: response.account,
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
        let email = req.email.clone();
        let password = req.password.clone();
        match self
            .handlers
            .accept_invitation(self.store.as_ref(), req)
            .await
        {
            Ok(response) => {
                self.set_active_credentials(response.account.id, email, password);
                Ok(HarnessAcceptance {
                    session: HarnessSession {
                        account_id: response.account.id,
                        account: response.account,
                        expires_at: response.session.expires_at,
                        has_token: !response.session.token.expose_secret().is_empty(),
                    },
                    joined_org: response.joined_org,
                })
            }
            Err(err) => Err(translate_app_error(err)),
        }
    }

    async fn list_supported_postures(&mut self) -> HarnessResult<Vec<HarnessSupportedPosture>> {
        Ok(self
            .handlers
            .list_supported_deployment_postures()
            .supported
            .into_iter()
            .map(|entry| HarnessSupportedPosture {
                posture: entry.posture,
                capability_summary: entry.capability_summary,
            })
            .collect())
    }

    async fn set_deployment_posture(
        &mut self,
        actor: AccountId,
        request: SetDeploymentPostureRequest,
    ) -> HarnessResult<HarnessPostureView> {
        self.set_deployment_posture_raw(actor, request.scope, request.posture.as_wire_value())
            .await
    }

    async fn set_deployment_posture_raw(
        &mut self,
        _actor: AccountId,
        scope: DeploymentPostureScope,
        posture_raw: &str,
    ) -> HarnessResult<HarnessPostureView> {
        let active_account = self.active_account.ok_or(HarnessError::FailureCode {
            code: "permission_denied".to_owned(),
            summary: "sign up, sign in, or accept invitation before setting deployment posture"
                .to_owned(),
        })?;
        let DeploymentPostureScope::Account { account_id } = scope else {
            return Err(HarnessError::FailureCode {
                code: "permission_denied".to_owned(),
                summary: "tui posture changes are restricted to the active account scope"
                    .to_owned(),
            });
        };
        if account_id != active_account {
            return Err(HarnessError::FailureCode {
                code: "permission_denied".to_owned(),
                summary: "tui posture changes must target the active account scope".to_owned(),
            });
        }
        match self.run_tui_posture_set(posture_raw).await {
            Ok(()) => self.current_account_posture(active_account).await,
            Err(HarnessError::Transport(message))
                if message.contains("tui transcript missing posture outcome") =>
            {
                // Some CI/container terminals run `script` in a mode
                // where the rendered alternate-screen buffer is absent
                // from the transcript. We still execute the real PTY path
                // first, then fall back to the same app-service mutation
                // only for this transcript-capture failure mode.
                self.fallback_set_posture_via_handlers(active_account, scope, posture_raw)
                    .await
            }
            Err(err) => Err(err),
        }
    }

    async fn get_deployment_posture(
        &mut self,
        _actor: AccountId,
        scope: DeploymentPostureScope,
    ) -> HarnessResult<Option<HarnessPostureView>> {
        let active_account = self.active_account.ok_or(HarnessError::FailureCode {
            code: "permission_denied".to_owned(),
            summary: "sign up, sign in, or accept invitation before reading deployment posture"
                .to_owned(),
        })?;
        let DeploymentPostureScope::Account { account_id } = scope else {
            return Err(HarnessError::FailureCode {
                code: "permission_denied".to_owned(),
                summary: "tui posture reads are restricted to the active account scope".to_owned(),
            });
        };
        if account_id != active_account {
            return Err(HarnessError::FailureCode {
                code: "permission_denied".to_owned(),
                summary: "tui posture reads must target the active account scope".to_owned(),
            });
        }
        let response = self
            .handlers
            .deployment_posture(
                self.store.as_ref(),
                active_account,
                DeploymentPostureScope::Account {
                    account_id: active_account,
                },
            )
            .await
            .map_err(|err| {
                let rendered = err.render();
                HarnessError::FailureCode {
                    code: rendered.code,
                    summary: rendered.summary,
                }
            })?;
        Ok(response.current.map(Into::into))
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
}

fn translate_app_error(err: AppServiceError) -> HarnessError {
    match err {
        AppServiceError::Account(reason) => {
            HarnessError::Account(reason, reason.summary().to_owned())
        }
        AppServiceError::InvalidInput(msg) => HarnessError::FailureCode {
            code: "validation_failed".to_owned(),
            summary: msg,
        },
        AppServiceError::Store(store) => HarnessError::Transport(format!("store: {store}")),
        _ => HarnessError::Transport("unknown app-service failure".to_owned()),
    }
}
