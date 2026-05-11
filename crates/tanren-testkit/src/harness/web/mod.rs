//! `@web` harness — real HTTP driver that spawns the API server and
//! drives it via `reqwest::Client` with cookie session and CSRF.
//!
//! This harness uses the same HTTP surface as `ApiHarness` (spawns
//! `tanren-api-app` on an ephemeral port, drives sign-up / sign-in /
//! role operations via `reqwest`) but is identified as
//! `HarnessKind::Web` to distinguish the coverage tag. Full
//! browser-rendering verification (DOM, JavaScript, CORS origin)
//! is owned by the Playwright suite in `apps/web/tests/bdd/`.
//!
//! Before this module existed, the `@web` tag silently delegated to
//! `InProcessHarness`, which provided no HTTP-level proof. The
//! current implementation drives real HTTP with cookie jars and CSRF
//! tokens so the Rust BDD runner exercises the same handlers the web
//! frontend calls.

mod role;

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use tanren_app_services::Store;
use tanren_contract::{AcceptInvitationRequest, AccountView, SignInRequest, SignUpRequest};
use tanren_identity_policy::AccountId;
use tanren_store::{AccountStore, EventEnvelope, NewInvitation};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use super::api::{
    accept_invitation_body, failure_from_body, scenario_db_path, sign_in_body, sign_up_body,
    sqlite_url,
};
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind, HarnessResult,
    HarnessSession,
};

/// `@web` wire harness — real HTTP driver against the API server.
pub struct WebHarness {
    base_url: String,
    client: Client,
    store: Arc<Store>,
    role_actor: Option<AccountId>,
    csrf_token: Option<String>,
    server: Option<JoinHandle<()>>,
    db_path: PathBuf,
}

impl std::fmt::Debug for WebHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebHarness")
            .field("base_url", &self.base_url)
            .field("db_path", &self.db_path)
            .finish_non_exhaustive()
    }
}

impl WebHarness {
    /// Spawn a fresh `tanren-api-app` on an ephemeral port against a
    /// per-scenario `SQLite` database file.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be connected /
    /// migrated, the listener cannot bind, or the api app cannot be
    /// constructed.
    pub async fn spawn() -> HarnessResult<Self> {
        let db_path = scenario_db_path("web");
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

        let cors_origin = axum::http::HeaderValue::from_str(&base_url)
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

        let client = Client::builder()
            .cookie_store(true)
            .timeout(super::HARNESS_DEFAULT_TIMEOUT)
            .build()
            .map_err(|e| HarnessError::Transport(format!("client build: {e}")))?;

        Ok(Self {
            base_url,
            client,
            store,
            role_actor: None,
            csrf_token: None,
            server: Some(server),
            db_path,
        })
    }
}

impl Drop for WebHarness {
    fn drop(&mut self) {
        if let Some(handle) = self.server.take() {
            handle.abort();
        }
        let _ = std::fs::remove_file(&self.db_path);
    }
}

#[async_trait]
impl AccountHarness for WebHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Web
    }

    async fn sign_up(&mut self, req: SignUpRequest) -> HarnessResult<HarnessSession> {
        let body = sign_up_body(&req);
        let url = format!("{}/accounts", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("POST /accounts: {e}")))?;
        let status = response.status();
        let cookies_set = response
            .headers()
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .count()
            > 0;
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        let account: AccountView = serde_json::from_value(json["account"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode account: {e}")))?;
        let session = &json["session"];
        let expires_at = session["expires_at"]
            .as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&chrono::Utc))
            .ok_or_else(|| HarnessError::Transport("missing session.expires_at".to_owned()))?;
        let csrf = session["csrf_token"].as_str().map(str::to_owned);
        self.csrf_token = csrf;
        self.role_actor = Some(account.id);
        Ok(HarnessSession {
            account_id: account.id,
            account,
            expires_at,
            has_token: cookies_set,
        })
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        let body = sign_in_body(&req);
        let url = format!("{}/accounts/session", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("POST /accounts/session: {e}")))?;
        let status = response.status();
        let cookies_set = response
            .headers()
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .count()
            > 0;
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        let account: AccountView = serde_json::from_value(json["account"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode account: {e}")))?;
        let session = &json["session"];
        let expires_at = session["expires_at"]
            .as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&chrono::Utc))
            .ok_or_else(|| HarnessError::Transport("missing session.expires_at".to_owned()))?;
        let csrf = session["csrf_token"].as_str().map(str::to_owned);
        self.csrf_token = csrf;
        self.role_actor = Some(account.id);
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
        let url = format!("{}/accounts/accept-invitation", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                HarnessError::Transport(format!("POST /accounts/accept-invitation: {e}"))
            })?;
        let status = response.status();
        let cookies_set = response
            .headers()
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .count()
            > 0;
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        let account: AccountView = serde_json::from_value(json["account"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode account: {e}")))?;
        let session = &json["session"];
        let expires_at = session["expires_at"]
            .as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&chrono::Utc))
            .ok_or_else(|| HarnessError::Transport("missing session.expires_at".to_owned()))?;
        let csrf = session["csrf_token"].as_str().map(str::to_owned);
        self.csrf_token = csrf;
        self.role_actor = Some(account.id);
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
        let mut out = Vec::with_capacity(requests.len());
        for r in requests {
            out.push(self.accept_invitation(r).await);
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

    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<EventEnvelope>> {
        AccountStore::recent_events(self.store.as_ref(), limit)
            .await
            .map_err(|e| HarnessError::Transport(format!("recent_events: {e}")))
    }

    async fn drain(&mut self) {
        if let Some(handle) = self.server.take() {
            handle.abort();
            let _ = handle.await;
        }
    }
}
