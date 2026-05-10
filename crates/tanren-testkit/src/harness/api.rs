//! `@api` harness — spawns `tanren-api-app` on an ephemeral port and drives it
//! via `reqwest::Client` with `cookie_store(true)`.

mod wire;

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use axum::http::HeaderValue;
use reqwest::Client;
use serde_json::Value;
use tanren_app_services::Store;
use tanren_contract::{
    AcceptInvitationRequest, AccountView, SignInRequest, SignUpRequest, SignedInAccountView,
    SwitchActiveAccountRequest,
};
use tanren_identity_policy::AccountId;
use tanren_store::{AccountStore, EventEnvelope, NewInvitation};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use super::api_codec::{accept_invitation_body, failure_from_body, sign_in_body, sign_up_body};
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind, HarnessResult,
    HarnessSession, InvalidSessionKind,
};
pub(crate) use wire::{scenario_db_path, sqlite_url};
use wire::{send_with_retry, wait_for_server_ready};

const WINDOW_ID_HEADER: &str = "x-tanren-window-id";

/// `@api` wire harness.
pub struct ApiHarness {
    base_url: String,
    window_id: String,
    client: Client,
    store: Arc<Store>,
    server: Option<JoinHandle<()>>,
    /// `SQLite` file path; deleted on drop.
    db_path: PathBuf,
}

impl std::fmt::Debug for ApiHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiHarness")
            .field("base_url", &self.base_url)
            .field("db_path", &self.db_path)
            .finish_non_exhaustive()
    }
}

impl ApiHarness {
    /// Spawn a fresh `tanren-api-app` on an ephemeral port against a
    /// per-scenario `SQLite` database file.
    pub async fn spawn() -> HarnessResult<Self> {
        let db_path = scenario_db_path("api");
        let database_url = sqlite_url(&db_path);
        let store = Store::connect(&database_url)
            .await
            .map_err(|e| HarnessError::Transport(format!("connect store: {e}")))?;
        store
            .migrate()
            .await
            .map_err(|e| HarnessError::Transport(format!("migrate store: {e}")))?;
        let store = Arc::new(store);

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| HarnessError::Transport(format!("bind listener: {e}")))?;
        let local_addr = listener
            .local_addr()
            .map_err(|e| HarnessError::Transport(format!("local addr: {e}")))?;
        let base_url = format!("http://{local_addr}");

        let cors_origin = HeaderValue::from_str(&base_url)
            .map_err(|e| HarnessError::Transport(format!("cors header: {e}")))?;
        let app = tanren_api_app::build_app_with_store(
            store.clone(),
            &database_url,
            vec![cors_origin],
            false,
        )
        .await
        .map_err(|e| HarnessError::Transport(format!("build app: {e}")))?;

        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        wait_for_server_ready(&base_url).await?;

        let client = build_cookie_client()?;

        Ok(Self {
            base_url,
            window_id: "11111111-1111-4111-8111-111111111111".to_owned(),
            client,
            store,
            server: Some(server),
            db_path,
        })
    }
}

impl Drop for ApiHarness {
    fn drop(&mut self) {
        if let Some(handle) = self.server.take() {
            handle.abort();
        }
        let _ = std::fs::remove_file(&self.db_path);
    }
}

#[async_trait]
impl AccountHarness for ApiHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Api
    }

    async fn sign_up(&mut self, req: SignUpRequest) -> HarnessResult<HarnessSession> {
        let body = sign_up_body(&req);
        let url = format!("{}/accounts", self.base_url);
        let window_id = self.window_id.clone();
        let response = send_with_retry(
            || {
                self.client
                    .post(&url)
                    .header(WINDOW_ID_HEADER, &window_id)
                    .json(&body)
            },
            "POST /accounts",
        )
        .await?;
        let status = response.status();
        let cookies_set = response
            .headers()
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .any(|v| {
                v.to_str()
                    .ok()
                    .is_some_and(|s| s.starts_with("tanren_session="))
            });
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        let account: AccountView = serde_json::from_value(json["account"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode account: {e}")))?;
        let expires_at = json["session"]["expires_at"]
            .as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&chrono::Utc))
            .ok_or_else(|| HarnessError::Transport("missing session.expires_at".to_owned()))?;
        Ok(HarnessSession {
            account_id: account.id,
            account,
            expires_at,
            has_token: cookies_set,
        })
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        let body = sign_in_body(&req);
        let url = format!("{}/sessions", self.base_url);
        let window_id = self.window_id.clone();
        let response = send_with_retry(
            || {
                self.client
                    .post(&url)
                    .header(WINDOW_ID_HEADER, &window_id)
                    .json(&body)
            },
            "POST /sessions",
        )
        .await?;
        let status = response.status();
        let cookies_set = response
            .headers()
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .any(|v| {
                v.to_str()
                    .ok()
                    .is_some_and(|s| s.starts_with("tanren_session="))
            });
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        let account: AccountView = serde_json::from_value(json["account"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode account: {e}")))?;
        let expires_at = json["session"]["expires_at"]
            .as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&chrono::Utc))
            .ok_or_else(|| HarnessError::Transport("missing session.expires_at".to_owned()))?;
        Ok(HarnessSession {
            account_id: account.id,
            account,
            expires_at,
            has_token: cookies_set,
        })
    }

    async fn accept_invitation(
        &mut self,
        req: AcceptInvitationRequest,
    ) -> HarnessResult<HarnessAcceptance> {
        let body = accept_invitation_body(&req);
        let token = req.invitation_token.as_str().to_owned();
        let url = format!("{}/invitations/{token}/accept", self.base_url);
        let window_id = self.window_id.clone();
        let response = send_with_retry(
            || {
                self.client
                    .post(&url)
                    .header(WINDOW_ID_HEADER, &window_id)
                    .json(&body)
            },
            "POST /invitations/{token}/accept",
        )
        .await?;
        let status = response.status();
        let cookies_set = response
            .headers()
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .any(|v| {
                v.to_str()
                    .ok()
                    .is_some_and(|s| s.starts_with("tanren_session="))
            });
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        let account: AccountView = serde_json::from_value(json["account"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode account: {e}")))?;
        let expires_at = json["session"]["expires_at"]
            .as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&chrono::Utc))
            .ok_or_else(|| HarnessError::Transport("missing session.expires_at".to_owned()))?;
        let joined_org = serde_json::from_value(json["joined_org"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode joined_org: {e}")))?;
        Ok(HarnessAcceptance {
            session: HarnessSession {
                account_id: account.id,
                account,
                expires_at,
                has_token: cookies_set,
            },
            joined_org,
        })
    }

    async fn accept_invitations_concurrent(
        &mut self,
        requests: Vec<AcceptInvitationRequest>,
    ) -> Vec<HarnessResult<HarnessAcceptance>> {
        let base_url = self.base_url.clone();
        let mut handles = Vec::with_capacity(requests.len());
        for req in requests {
            let url = format!(
                "{}/invitations/{}/accept",
                base_url,
                req.invitation_token.as_str()
            );
            let body = accept_invitation_body(&req);
            let window_id = self.window_id.clone();
            let client = match Client::builder().build() {
                Ok(c) => c,
                Err(e) => {
                    handles.push(tokio::spawn(async move {
                        Err::<HarnessAcceptance, HarnessError>(HarnessError::Transport(format!(
                            "build client: {e}"
                        )))
                    }));
                    continue;
                }
            };
            handles.push(tokio::spawn(async move {
                let response = client
                    .post(&url)
                    .header(WINDOW_ID_HEADER, window_id)
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| {
                        HarnessError::Transport(format!("POST /invitations/{{token}}/accept: {e}"))
                    })?;
                let status = response.status();
                let cookies_set = response
                    .headers()
                    .get_all(reqwest::header::SET_COOKIE)
                    .iter()
                    .any(|v| {
                        v.to_str()
                            .ok()
                            .is_some_and(|s| s.starts_with("tanren_session="))
                    });
                let json: Value = response
                    .json()
                    .await
                    .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
                if !status.is_success() {
                    return Err(failure_from_body(&json));
                }
                let account: AccountView = serde_json::from_value(json["account"].clone())
                    .map_err(|e| HarnessError::Transport(format!("decode account: {e}")))?;
                let expires_at = json["session"]["expires_at"]
                    .as_str()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|d| d.with_timezone(&chrono::Utc))
                    .ok_or_else(|| {
                        HarnessError::Transport("missing session.expires_at".to_owned())
                    })?;
                let joined_org = serde_json::from_value(json["joined_org"].clone())
                    .map_err(|e| HarnessError::Transport(format!("decode joined_org: {e}")))?;
                Ok(HarnessAcceptance {
                    session: HarnessSession {
                        account_id: account.id,
                        account,
                        expires_at,
                        has_token: cookies_set,
                    },
                    joined_org,
                })
            }));
        }
        let mut out = Vec::with_capacity(handles.len());
        for h in handles {
            out.push(match h.await {
                Ok(r) => r,
                Err(e) => Err(HarnessError::Transport(format!("join: {e}"))),
            });
        }
        out
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

    async fn list_active_accounts(&mut self) -> HarnessResult<Vec<SignedInAccountView>> {
        let window_id = self.window_id.clone();
        self.list_active_accounts_with_window(Some(window_id.as_str()))
            .await
    }

    async fn list_active_accounts_in_window(
        &mut self,
        window_id: &str,
    ) -> HarnessResult<Vec<SignedInAccountView>> {
        self.list_active_accounts_with_window(Some(window_id)).await
    }

    async fn switch_active_account(
        &mut self,
        target_account_id: AccountId,
    ) -> HarnessResult<Vec<SignedInAccountView>> {
        let window_id = self.window_id.clone();
        self.switch_active_account_with_window(Some(window_id.as_str()), target_account_id)
            .await
    }

    async fn switch_active_account_in_window(
        &mut self,
        window_id: &str,
        target_account_id: AccountId,
    ) -> HarnessResult<Vec<SignedInAccountView>> {
        self.switch_active_account_with_window(Some(window_id), target_account_id)
            .await
    }

    async fn invalidate_caller_session(&mut self, mode: InvalidSessionKind) -> HarnessResult<()> {
        match mode {
            InvalidSessionKind::Missing | InvalidSessionKind::Expired => {
                self.client = build_cookie_client()?;
                Ok(())
            }
            InvalidSessionKind::Revoked => {
                let url = format!("{}/sessions/revoke", self.base_url);
                let window_id = self.window_id.clone();
                let response = send_with_retry(
                    || self.client.post(&url).header(WINDOW_ID_HEADER, &window_id),
                    "POST /sessions/revoke",
                )
                .await?;
                if response.status().is_success() {
                    Ok(())
                } else {
                    Err(HarnessError::Transport(format!(
                        "revoke session failed with status {}",
                        response.status()
                    )))
                }
            }
        }
    }

    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<EventEnvelope>> {
        AccountStore::recent_events(self.store.as_ref(), limit)
            .await
            .map_err(|e| HarnessError::Transport(format!("recent_events: {e}")))
    }
}

impl ApiHarness {
    async fn list_active_accounts_with_window(
        &mut self,
        window_id: Option<&str>,
    ) -> HarnessResult<Vec<SignedInAccountView>> {
        let url = format!("{}/accounts/active", self.base_url);
        let response = send_with_retry(
            || {
                let mut request = self.client.get(&url);
                if let Some(window_id) = window_id.filter(|id| !id.trim().is_empty()) {
                    request = request.header(WINDOW_ID_HEADER, window_id);
                }
                request
            },
            "GET /accounts/active",
        )
        .await?;
        let status = response.status();
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        serde_json::from_value(json["accounts"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode active accounts: {e}")))
    }

    async fn switch_active_account_with_window(
        &mut self,
        window_id: Option<&str>,
        target_account_id: AccountId,
    ) -> HarnessResult<Vec<SignedInAccountView>> {
        let url = format!("{}/accounts/active/switch", self.base_url);
        let response = send_with_retry(
            || {
                let mut request = self
                    .client
                    .post(&url)
                    .json(&SwitchActiveAccountRequest { target_account_id });
                if let Some(window_id) = window_id.filter(|id| !id.trim().is_empty()) {
                    request = request.header(WINDOW_ID_HEADER, window_id);
                }
                request
            },
            "POST /accounts/active/switch",
        )
        .await?;
        let status = response.status();
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        serde_json::from_value(json["accounts"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode active accounts: {e}")))
    }
}

fn build_cookie_client() -> HarnessResult<Client> {
    Client::builder()
        .cookie_store(true)
        .timeout(super::HARNESS_DEFAULT_TIMEOUT)
        .build()
        .map_err(|e| HarnessError::Transport(format!("client build: {e}")))
}
